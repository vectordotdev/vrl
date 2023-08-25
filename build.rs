extern crate lalrpop;

use std::{
    env,
    fmt::Write as fmt_write,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
};

fn main() {
    read_grok_patterns();

    println!("cargo:rerun-if-changed=src/parser/parser.lalrpop");
    lalrpop::Configuration::new()
        .always_use_colors()
        .process_dir("src/datadog/grok")
        .unwrap();

    lalrpop::Configuration::new()
        .always_use_colors()
        .emit_rerun_directives(true)
        .emit_whitespace(false)
        .process_dir("src/parser")
        .unwrap();
}

/// Reads grok patterns defined in the `patterns` folder into the static `PATTERNS` variable
fn read_grok_patterns() {
    let mut output =
        "#[allow(clippy::needless_raw_string_hashes)]\nstatic PATTERNS: &[(&str, &str)] = &[\n"
            .to_string();

    fs::read_dir(Path::new("src/datadog/grok/patterns"))
        .expect("can't read 'patterns' dir")
        .filter_map(|path| File::open(path.expect("can't read 'patterns' dir").path()).ok())
        .flat_map(|f| BufReader::new(f).lines().map_while(Result::ok))
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .for_each(|line| {
            let (key, value) = line.split_at(
                line.find(' ')
                    .expect("pattern should follow the format 'ruleName definition'"),
            );
            write!(output, "\t(\"{}\", r#\"{}\"#),", key, &value[1..])
                .expect("can't read pattern definitions");
        });

    output.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR isn't defined");
    let dest_path = Path::new(&out_dir).join("patterns.rs");
    fs::write(dest_path, output).expect("'patterns.rs' wasn't generated");
}
