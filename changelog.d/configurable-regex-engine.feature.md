Add an optional PCRE2 JIT execution backend for all VRL regex operators. Embedders select the backend through `CompileConfig`; patterns use Rust syntax and capture metadata, while matching follows the selected engine. Nullable repetitions can produce different whole matches and captures, affecting iteration, splits, and replacements. Rust remains the default; select it when exact Rust matching behavior is required.

authors: splitice
