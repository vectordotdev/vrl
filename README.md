# Vector Remap Language (VRL)

This directory houses the Rust libraries used to power [Vector Remap Language][vrl], or **VRL** for short. VRL is a
language for transforming, routing, and filtering observability data (logs and metrics). Although VRL was originally
created for use in [Vector], in principle it can be used in other systems.

## Libraries

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
