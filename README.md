# Vector Remap Language (VRL)

[![Crates.io](https://img.shields.io/crates/v/vrl?style=flat-square)](https://crates.io/crates/vrl)
[![docs.rs](https://img.shields.io/docsrs/vrl?style=flat-square)](https://docs.rs/vrl/0.4.0/vrl/)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/vectordotdev/vrl/test.yml?style=flat-square)

VRL is a scripting language for processing observability data (logs, metrics, traces). Although VRL was originally
created for use in [Vector], it was designed to be generic and re-usable in many contexts.

VRL is maintained by
Datadog's [Community Open Source Engineering team](https://opensource.datadoghq.com/about/#the-community-open-source-engineering-team).

## Webassembly

All of the core features, and most of the standard library functions can be compiled with the `wasm32-unknown-unknown` target.
There are a few stdlib functions that are unsupported. These will still compile, but abort at runtime.

Unsupported functions:
- `parse_grok`
- `parse_groks`
- `log`
- `get_hostname`
- `reverse_dns`
- `http_request`



[vector]: https://vector.dev
[vrl]: https://vrl.dev
