"""Exercise Linux CA-store behavior through the VRL CLI without changing host trust."""

import contextlib
import http.server
import json
import os
from pathlib import Path
import ssl
import subprocess
import sys
import tempfile
import threading


class Handler(http.server.BaseHTTPRequestHandler):
    redirect_url = ""

    def do_GET(self):
        if self.path == "/redirect":
            self.send_response(302)
            self.send_header("Location", self.redirect_url)
            self.end_headers()
        else:
            body = b"http-request-ok"
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    def log_message(self, *args):
        pass


@contextlib.contextmanager
def server(tls=None):
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler) as instance:
        if tls is not None:
            instance.socket = tls.wrap_socket(instance.socket, server_side=True)
        thread = threading.Thread(target=instance.serve_forever, daemon=True)
        thread.start()
        try:
            yield instance.server_port
        finally:
            instance.shutdown()
            thread.join()


def run(binary, program, env):
    result = subprocess.run(
        [binary, program], input="{}\n", text=True, capture_output=True,
        env=env, timeout=90,
    )
    assert result.returncode == 0, (program, result.stdout, result.stderr)
    assert result.stdout.strip() == "true", (program, result.stdout, result.stderr)


def main():
    if not sys.platform.startswith("linux"):
        raise SystemExit("This test requires Linux certificate-file overrides")
    binary = str(Path(sys.argv[1]).resolve())
    with tempfile.TemporaryDirectory(prefix="vrl-http-tls-") as directory:
        root = Path(directory)
        key, cert = root / "key.pem", root / "cert.pem"
        subprocess.run(
            ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
             "-days", "1", "-subj", "/CN=localhost",
             "-addext", "basicConstraints=critical,CA:FALSE",
             "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
             "-keyout", str(key), "-out", str(cert)],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        empty_file, empty_dir = root / "empty.pem", root / "empty-dir"
        empty_file.touch()
        empty_dir.mkdir()
        env = {
            key: value for key, value in os.environ.items()
            if key.lower() not in {"http_proxy", "https_proxy", "all_proxy", "no_proxy"}
        }
        env.update(SSL_CERT_FILE=str(empty_file), SSL_CERT_DIR=str(empty_dir))
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.load_cert_chain(cert, key)
        with server(tls) as tls_port, server() as http_port:
            http = f"http://127.0.0.1:{http_port}"
            https = f"https://localhost:{tls_port}"
            Handler.redirect_url = https

            # The same TLS server succeeds with its CA trusted, so rejection below
            # cannot pass merely because the server is unreachable or misconfigured.
            trusted = dict(env, SSL_CERT_FILE=str(cert))
            run(binary, f'http_request!({json.dumps(https)}) == "http-request-ok"', trusted)

            # Repeated calls also exercise the cached default client after initialization.
            run(binary, f'assert_eq!(http_request!("{http}"), "http-request-ok"); '
                        f'http_request!("{http}") == "http-request-ok"', env)
            for argument in [f'http_proxy: "{http}"', "http_proxy: .proxy"]:
                run(binary, f'.proxy = "{http}"; '
                            f'http_request!("http://unresolvable.invalid", {argument}) '
                            '== "http-request-ok"', env)

            # Empty roots must fail closed for direct TLS, redirects, and TLS proxies.
            requests = [
                f'http_request("{https}")',
                f'http_request("{http}/redirect")',
                f'http_request("http://unresolvable.invalid", http_proxy: "{https}")',
                f'http_request("{https}", https_proxy: "{https}")',
            ]
            for request in requests:
                run(binary, f'response, err = {request}; '
                            'contains(to_string!(err), "HTTP request failed")', env)
        print("Passed: trusted HTTPS; empty-store HTTP and proxies; rejected HTTPS, redirects, and TLS proxies")


if __name__ == "__main__":
    main()
