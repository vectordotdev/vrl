# Vector Remap Language (VRL)

[![Crates.io](https://img.shields.io/crates/v/vrl?style=flat-square)](https://crates.io/crates/vrl)
[![docs.rs](https://img.shields.io/docsrs/vrl?style=flat-square)](https://docs.rs/vrl/latest/vrl/)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/vectordotdev/vrl/test.yml?style=flat-square)

VRL is an expression-oriented language for parsing, transforming, and enriching
observability data (logs, metrics, and traces). Originally created for [Vector],
it can also be embedded in other contexts.

VRL is maintained by Datadog's
[Community Open Source Engineering team][community-team].

## Quickstart

[Try VRL in your browser][playground]: write a program, run it against your
events, and inspect the results without installing anything.

[Language guide][guide] · [Function reference][functions] ·
[Examples][examples] · [Error reference][errors]

## Design

VRL is designed around these core principles:

- **Safety**: the compiler checks types and requires you to handle fallible
  expressions before a program can run.
- **Statelessness**: programs process each event independently, without retaining
  state between events.
- **Performance**: programs are compiled once and executed without garbage collection.
- **Reusability**: VRL is designed to be embedded in applications beyond Vector.

## Use VRL

- **Command line:**

  ```sh
  cargo run
  ```

- **In Vector:** use the [`remap` transform][remap] to modify events, or VRL
  conditions to [filter][filter] and [route][route] them.
- **In Rust:** start with the [embedding example](examples/simple.rs) and the
  [Rust API documentation][rust-api].

## WebAssembly

VRL can be compiled with the `wasm32-unknown-unknown` target:

```sh
cargo check --target wasm32-unknown-unknown --no-default-features --features stdlib
```

Most stdlib functions are supported. The following functions compile but abort
at runtime due to platform limitations (I/O, system calls, or native dependencies):

- `dns_lookup`
- `get_hostname`
- `http_request`
- `log`
- `parse_grok`
- `parse_groks`
- `reverse_dns`
- `validate_json_schema`

Note: the grok parser within the `datadog` feature is excluded entirely when
targeting wasm32.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development and testing instructions,
and [DESIGN.md](DESIGN.md) for the language's design principles. Please read the
[AI policy](AI_POLICY.md) when contributing with AI tools.

[vector]: https://vector.dev
[playground]: https://playground.vrl.dev/
[guide]: https://vector.dev/docs/reference/vrl/
[functions]: https://vector.dev/docs/reference/vrl/functions/
[examples]: https://vector.dev/docs/reference/vrl/examples/
[errors]: https://vector.dev/docs/reference/vrl/errors/
[remap]: https://vector.dev/docs/reference/configuration/transforms/remap/
[filter]: https://vector.dev/docs/reference/configuration/transforms/filter/
[route]: https://vector.dev/docs/reference/configuration/transforms/route/
[rust-api]: https://docs.rs/vrl/latest/vrl/
[community-team]: https://opensource.datadoghq.com/about/#the-community-open-source-engineering-team
