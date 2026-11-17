Preserve compiled Rust regex builder options when PCRE2 JIT is selected by retaining the original Rust execution for imported regex values. Keep `match_any` byte-set matching on the Rust engine for non-ASCII input, including matches inside UTF-8 characters.

authors: splitice
