# Vector Remap Language (VRL)

[![Crates.io](https://img.shields.io/crates/v/vrl?style=flat-square)](https://crates.io/crates/vrl)
[![docs.rs](https://img.shields.io/docsrs/vrl?style=flat-square)](https://docs.rs/vrl/0.4.0/vrl/)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/vectordotdev/vrl/test.yml?style=flat-square)

VRL is a scripting language for processing observability data (logs, metrics, traces). Although VRL was originally
created for use in [Vector], it was designed to be generic and re-usable in many contexts.

VRL is designed around two core principles:

- **Safety** — programs won't compile unless all errors from fallible functions are explicitly handled, eliminating unexpected runtime
  failures.
- **Performance** — programs are compiled at startup and run with near-native performance, with no garbage collection or runtime overhead.

VRL is stateless and expression-oriented, each program processes a single event and every expression returns a value.

VRL is maintained by
Datadog's [Community Open Source Engineering team](https://opensource.datadoghq.com/about/#the-community-open-source-engineering-team).

## WebAssembly

VRL can be compiled with the `wasm32-unknown-unknown` target:

```sh
cargo check --target wasm32-unknown-unknown --no-default-features --features stdlib
```

Most stdlib functions are supported. The following functions compile but abort at runtime due to platform limitations (I/O, system calls, or native dependencies):

- `dns_lookup`
- `get_hostname`
- `http_request`
- `log`
- `parse_grok`
- `parse_groks`
- `reverse_dns`
- `validate_json_schema`

Note: the `datadog_grok` feature is excluded entirely when targeting wasm32.

[vector]: https://vector.dev
[vrl]: https://vrl.dev
