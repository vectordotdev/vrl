Add an optional PCRE2 JIT execution backend for all VRL regex operators. Embedders select the backend through `CompileConfig`; Rust syntax and value semantics remain authoritative, and Rust remains the default.

authors: splitice
