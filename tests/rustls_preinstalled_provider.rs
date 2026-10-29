#![cfg(all(
    feature = "stdlib",
    feature = "enable_network_functions",
    not(target_arch = "wasm32")
))]

// Runs in its own test binary because a process-level rustls provider can only be installed once.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use rustls::crypto::CryptoProvider;
use vrl::compiler::{Context, TargetValue, TimeZone, state::RuntimeState};
use vrl::value;
use vrl::value::Secrets;

fn start_mock_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        }
    });

    port
}

#[test]
fn http_request_preserves_preinstalled_provider() {
    let mut provider = rustls::crypto::ring::default_provider();
    provider.cipher_suites.truncate(1);
    provider.install_default().unwrap();
    let installed = Arc::clone(CryptoProvider::get_default().unwrap());

    let port = start_mock_server();
    let src = format!(r#"http_request!("http://127.0.0.1:{port}/")"#);
    let program = vrl::compiler::compile(&src, &vrl::stdlib::all())
        .unwrap()
        .program;

    let mut target = TargetValue {
        value: value!({}),
        metadata: value!({}),
        secrets: Secrets::default(),
    };
    let mut state = RuntimeState::default();
    let timezone = TimeZone::default();
    let mut ctx = Context::new(&mut target, &mut state, &timezone);
    assert_eq!(program.resolve(&mut ctx).unwrap(), value!("ok"));

    let current = CryptoProvider::get_default().unwrap();
    assert!(Arc::ptr_eq(&installed, current));
    assert_eq!(current.cipher_suites.len(), 1);
}
