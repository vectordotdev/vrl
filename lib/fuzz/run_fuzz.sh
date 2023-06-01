set -e
./split_input.sh
cargo afl build --release
cargo afl fuzz -i in -o out ../../target/release/fuzz