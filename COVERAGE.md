# Code Coverage

## Quick Start

```bash
# Install cargo-llvm-cov (first time only)
cargo install cargo-llvm-cov

# Run coverage and open HTML report
cargo llvm-cov --html --open

# Run coverage for specific tests
cargo llvm-cov --html --open -- test_name
```

## Reports

HTML report location: `target/llvm-cov/html/index.html`

## Coverage Summary

View text summary:
```bash
cargo llvm-cov report --summary-only
```

## Notes

- Trybuild compile-time tests do not appear in coverage reports (they verify compile errors, not runtime behavior)
- Coverage measures runtime test execution only
- Current project coverage: ~74% lines, ~68% functions

## Reference

- [cargo-llvm-cov documentation](https://github.com/taiki-e/cargo-llvm-cov)
