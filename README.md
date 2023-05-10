# Vector Remap Language (VRL)

VRL is a scripting language for processing observability data (logs, metrics, traces). Although VRL was originally
created for use in [Vector], it was designed to be generic and re-usable in many contexts.


## Features

VRL is broken up into multiple components, which can be enabled as needed.

| Feature        | Default?| Description                                                                                      |
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


| Library                            | Purpose                                                                                                                               |
|:-----------------------------------|:--------------------------------------------------------------------------------------------------------------------------------------|
| [`vrl-cli`](lib/cli)               | VRL has a command-line interface that can be used either under the `vector` CLI (`vector vrl`) or on its own via `cargo run`          |
| [`vrl-compiler`](lib/compiler)     | The VRL compiler converts a system of VRL expressions (parsed from a VRL program) into runnable Rust code                             |
| [`vrl-core`](lib/core)             | Some core bits for the language, including the `Target` trait that needs to be implemented by events                                  |
| [`vrl-diagnostic`](lib/diagnostic) | Compiler and runtime error messages as well as runtime error logging                                                                  |
| [`vrl-parser`](lib/parser)         | The VRL parser uses an abstract syntax tree (AST) to convert VRL programs inside of Vector configurations into systems of expressions |
| [`vrl-proptests`](lib/proptests)   | A collection of property-based tests for VRL parser                                                                                   |
| [`vrl-stdlib`](lib/stdlib)         | The current standard library of VRL functions                                                                                         |
| [`vrl-tests`](lib/tests)           | A harness used to run a variety of test cases                                                                                         |
| [`vrl`](.)                         | A fully packaged version of VRL, bundling together all internal components into a public interface                                    |

[vector]: https://vector.dev
[vrl]: https://vrl.dev
