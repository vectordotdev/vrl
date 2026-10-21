Updated `rustls-webpki` to 0.103.16 to address [RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382). TLS certificate verification in `http_request` now rejects certificate chains that contain the compromised eMudhra emSign intermediate certificates, even when the system trust store still trusts their root.

authors: thomasqueirozb
