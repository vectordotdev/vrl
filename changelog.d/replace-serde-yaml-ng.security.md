Replaced the `serde_yaml_ng` crate with `serde-saphyr`, a pure-Rust YAML parser, for the `parse_yaml` function and build-time user-agent regex parsing. This removes the unmaintained `unsafe-libyaml` crate (RUSTSEC-2024-0449) from the dependency tree entirely. Error messages reported by `parse_yaml` for malformed YAML now use the new parser's format, which includes source snippets.

authors: shrey-aria
