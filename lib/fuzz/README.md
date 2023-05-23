# VRL Fuzz Tester

This uses AFL (American Fuzzy Lop) to generate input that will try to make the test program fail (panic / crash / freeze / etc.).

The test program (`main.rs`) will take the input as VRL source code, compile it, run the program, and then run some
sanity checks / asserts on the result.

## How to run
- Install AFL (`cargo install afl`). Make sure this is installed in the VRL directory, to ensure it's using the same Rust version that that test program will be compiled against (due to the `rust-toolchain.toml`);
- Add any additional starting inputs as desired to `inputs.txt`. These should all be VALID vrl code. This helps AFL find "interesting" code faster. Each line is treated as a separate input. Do not modify files in the `in/` directory, those are generated automatically.
- Run the fuzzer with `./run_fuzz.sh`. This will run forever. Just let it run for as long as you are willing (it could take hours or days to find a crash).
- For an explanation of what the status screen means, you can read more about it here: https://lcamtuf.coredump.cx/afl/status_screen.txt
- the `out` directory will be populated with any issues found. The `crashes` or `hangs` directory inside here will contain files of the VRL input used to cause the failure. You can usually just take this input and stick it in the REPL to see what's going on. In the future, extra tooling may be provided to tell you exactly what happened.
- If any crashes are unique / currently unknown, open a github issue on the VRL repo. Make sure to include the `fuzz` tag!

## Troubleshooting / Known issues
- If you get an error about "on-demand CPU frequency scaling" and don't want to follow the instructions to fix it, you can just set the `AFL_SKIP_CPUFREQ` environment variable to `true` to skip this step. Fuzzing will be a bit slower.
- The scripts provided only run AFL on a single core. This can be improved in the future to utilize more cores.