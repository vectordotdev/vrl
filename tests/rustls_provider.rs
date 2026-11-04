#![cfg(all(feature = "enable_network_functions", not(target_arch = "wasm32")))]

// Downstream crates call `ClientConfig::builder()` without installing a provider. That panics
// when more than one rustls crypto provider is enabled, so VRL must only enable ring.
#[test]
fn rustls_crate_default_provider_is_unambiguous() {
    let _ = rustls::ClientConfig::builder();
}
