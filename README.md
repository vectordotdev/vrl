# Vector Remap Language (VRL)

[![Crates.io](https://img.shields.io/crates/v/vrl?style=flat-square)](https://crates.io/crates/vrl)
[![docs.rs](https://img.shields.io/docsrs/vrl?style=flat-square)](https://docs.rs/vrl/0.4.0/vrl/)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/vectordotdev/vrl/test.yml?style=flat-square)

VRL is a scripting language for processing observability data (logs, metrics, traces). Although VRL was originally
created for use in [Vector], it was designed to be generic and re-usable in many contexts.


## Features

VRL is broken up into multiple components, which can be enabled as needed.

| Feature        | Default | Description                                                                                      |
|:---------      |:--------|:----------                                                                                       |
| compiler       | yes     | The contains the core functionality of VRL. Compiling and running VRL programs.                  |
| parser         | yes     | Creates an abstract syntax tree (AST) from VRL source code.                                      |
| value          | yes     | Contains the primary data type used in VRL.                                                      |
| diagnostic     | yes     | Logic related to errors and displaying info about them.                                          |
| path           | yes     | Contains the parser, datatypes, and functions related to VRL paths.                              |
| stdlib         | yes     | All of the VRL functions from the standard library.                                              |
| core           | yes     | Various data structures and utility methods (these may be renamed / moved in the future).        |
| datadog_filter | yes     | Implements the Datadog log search query filter syntax.                                           |
| datadog_grok   | yes     | Implements the Datadog grok parser. (used with `parse_grok` and `parse_groks` in the stdlib).    |
| datadog_search | yes     | Implements the Datadog log search syntax.                                                        |
| cli            | no      | Contains functionality to create a CLI for VRL.                                                  |
| test_framework | no      | Contains the test framework for testing VRL functions. Useful for testing custom functions.      |
| lua            | no      | Makes the `Value` type compatible with the `mlua` crate.                                         |
| arbitrary      | no      | Implements `Arbitrary` (from the `quickcheck` crate) for the `Value` type                        |


## Webassembly

All of the core features, and most of the standard library functions can be compiled with the `wasm32-unknown-unknown` target.
There are a few stdlib functions that are unsupported. These will still compile, but abort at runtime.

Unsupported functions:
- `parse_grok`
- `parse_groks`
- `log`
- `get_hostname`
- `reverse_dns`



[vector]: https://vector.dev
[vrl]: https://vrl.dev
