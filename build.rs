extern crate lalrpop;

use std::{
    borrow::Cow,
    env,
    fmt::Write as fmt_write,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::Path,
};
use ua_parser::device::Flag;

fn main() {
    read_grok_patterns();

    #[cfg(feature = "stdlib")]
    convert_user_agent_regexes();

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

    let mut files: Vec<_> = fs::read_dir(Path::new("src/datadog/grok/patterns"))
        .expect("can't read 'patterns' dir")
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .expect("cant't read 'patterns' dir");
    files.sort();
    files
        .into_iter()
        .filter_map(|path| File::open(path).ok())
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

#[cfg(feature = "stdlib")]
fn convert_user_agent_regexes() {
    let regexes = fs::read("data/user_agent_regexes.yaml").expect("Could not read regexes");
    let regexes: ua_parser::Regexes =
        serde_yaml::from_slice(&regexes).expect("Regex file is not valid yaml");

    fn write_item(output: &mut Vec<u8>, name: &'static str, value: Option<Cow<str>>) {
        if let Some(value) = value {
            output.extend(format!("    {}: Some(r#\"{}\"#.into()),\n", name, value).bytes());
        } else {
            output.extend(format!("    {}: None,\n", name).bytes());
        }
    }

    let mut output = Vec::new();

    output.extend(b"ua_parser::Regexes {\n");

    output.extend(b"os_parsers: vec![\n");
    for os in regexes.os_parsers {
        output.extend(b"#[allow(clippy::needless_raw_string_hashes)]\n");
        output.extend(b"ua_parser::os::Parser {\n");
        output.extend(format!("    regex: r#\"{}\"#.into(),\n", os.regex).bytes());
        write_item(&mut output, "os_replacement", os.os_replacement);
        write_item(&mut output, "os_v1_replacement", os.os_v1_replacement);
        write_item(&mut output, "os_v2_replacement", os.os_v2_replacement);
        write_item(&mut output, "os_v3_replacement", os.os_v3_replacement);
        write_item(&mut output, "os_v4_replacement", os.os_v4_replacement);
        output.extend(b"},\n");
    }
    output.extend(b"],\n");

    output.extend(b"user_agent_parsers: vec![\n");
    for ua in regexes.user_agent_parsers {
        output.extend(b"#[allow(clippy::needless_raw_string_hashes)]\n");
        output.extend(b"ua_parser::user_agent::Parser {\n");
        output.extend(format!("    regex: r#\"{}\"#.into(),\n", ua.regex).bytes());
        write_item(&mut output, "family_replacement", ua.family_replacement);
        write_item(&mut output, "v1_replacement", ua.v1_replacement);
        write_item(&mut output, "v2_replacement", ua.v2_replacement);
        write_item(&mut output, "v3_replacement", ua.v3_replacement);
        write_item(&mut output, "v4_replacement", ua.v4_replacement);
        output.extend(b"},\n");
    }
    output.extend(b"],\n");

    output.extend(b"device_parsers: vec![\n");
    for device in regexes.device_parsers {
        output.extend(b"#[allow(clippy::needless_raw_string_hashes)]\n");
        output.extend(b"ua_parser::device::Parser {\n");
        output.extend(format!("    regex: r#\"{}\"#.into(),\n", device.regex).bytes());
        match device.regex_flag {
            Some(Flag::IgnoreCase) => {
                output.extend(b"    regex_flag: Some(ua_parser::device::Flag::IgnoreCase),\n");
            }
            None => {
                output.extend(b"    regex_flag: None,\n");
            }
        }
        write_item(&mut output, "device_replacement", device.device_replacement);
        write_item(&mut output, "brand_replacement", device.brand_replacement);
        write_item(&mut output, "model_replacement", device.model_replacement);
        output.extend(b"},\n");
    }
    output.extend(b"],\n}\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR isn't defined");
    let dest_path = Path::new(&out_dir).join("user_agent_regexes.rs");
    fs::write(dest_path, output).expect("'user_agent_regexes.rs' wasn't generated");
}
