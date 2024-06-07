#![deny(warnings)]
#![allow(clippy::print_stdout)] // tests
#![allow(clippy::print_stderr)] // tests

use std::path::{PathBuf, MAIN_SEPARATOR};
use std::{collections::BTreeMap, env, str::FromStr, time::Instant};

use ansi_term::Colour;
use chrono::{DateTime, SecondsFormat, Utc};

pub use test::Test;

use crate::compiler::{
    compile_with_external,
    runtime::{Runtime, Terminate},
    state::{ExternalEnv, RuntimeState},
    value::VrlValueConvert,
    CompilationResult, CompileConfig, Function, Program, SecretTarget, TargetValueRef, TimeZone,
    VrlRuntime,
};
use crate::diagnostic::{DiagnosticList, Formatter};
use crate::value::Secrets;
use crate::value::Value;

#[allow(clippy::module_inception)]
mod test;

fn measure_time<F, R>(f: F) -> (R, std::time::Duration)
where
    F: FnOnce() -> R, // F is a closure that takes no argument and returns a value of type R
{
    let start = Instant::now();
    let result = f(); // Execute the closure
    let duration = start.elapsed();
    (result, duration) // Return the result of the closure and the elapsed time
}

pub struct TestConfig {
    pub fail_early: bool,
    pub verbose: bool,
    pub no_diff: bool,
    pub timings: bool,
    pub runtime: VrlRuntime,
    pub timezone: TimeZone,
}

pub fn test_dir() -> PathBuf {
    PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
}

pub fn test_prefix() -> String {
    let mut prefix = test_dir().join("tests").to_string_lossy().to_string();
    prefix.push(MAIN_SEPARATOR);
    prefix
}

pub fn example_vrl_path() -> PathBuf {
    test_dir().join("tests").join("example.vrl")
}

pub fn get_tests_from_functions(functions: Vec<Box<dyn Function>>) -> Vec<Test> {
    let mut tests = vec![];
    functions.into_iter().for_each(|function| {
        if let Some(closure) = function.closure() {
            closure.inputs.iter().for_each(|input| {
                let test = Test::from_example(
                    format!("{} (closure)", function.identifier()),
                    &input.example,
                );
                tests.push(test);
            });
        }

        function.examples().iter().for_each(|example| {
            let test = Test::from_example(function.identifier(), example);
            tests.push(test)
        })
    });

    tests
}

pub fn run_tests<T>(
    tests: Vec<Test>,
    cfg: &TestConfig,
    functions: &[Box<dyn Function>],
    compile_config_provider: impl Fn() -> (CompileConfig, T),
    finalize_config: impl Fn(T),
) {
    let total_count = tests.len();
    let mut failed_count = 0;
    let mut warnings_count = 0;
    let mut category = "".to_owned();

    for mut test in tests {
        if category != test.category {
            category.clone_from(&test.category);
            println!("{}", Colour::Fixed(3).bold().paint(category.to_string()));
        }

        if let Some(err) = test.error {
            println!("{}", Colour::Purple.bold().paint("INVALID"));
            println!("{}", Colour::Red.paint(err));
            failed_count += 1;
            continue;
        }

        let mut name = test.name.clone();
        name.truncate(58);

        let dots = if name.len() >= 60 { 0 } else { 60 - name.len() };
        print!("  {}{}", name, Colour::Fixed(240).paint(".".repeat(dots)));

        if test.skip {
            println!("{}", Colour::Yellow.bold().paint("SKIPPED"));
            continue;
        }

        let (mut config, config_metadata) = (compile_config_provider)();
        // Set some read-only paths that can be tested
        for (path, recursive) in &test.read_only_paths {
            config.set_read_only_path(path.clone(), *recursive);
        }

        let (result, compile_duration) = measure_time(|| {
            compile_with_external(&test.source, functions, &ExternalEnv::default(), config)
        });
        let compile_timing_fmt = cfg
            .timings
            .then(|| format!("comp: {:>9.3?}", compile_duration))
            .unwrap_or_default();

        let failed = match result {
            Ok(CompilationResult {
                program,
                warnings,
                config: _,
            }) => {
                warnings_count += warnings.len();

                if test.check_diagnostics {
                    process_compilation_diagnostics(&test, cfg, warnings, compile_timing_fmt)
                } else if warnings.is_empty() {
                    let run_start = Instant::now();

                    finalize_config(config_metadata);
                    let result = run_vrl(program, &mut test.object, cfg.timezone, cfg.runtime);
                    let run_end = run_start.elapsed();

                    let timings = {
                        let timings_color = if run_end.as_millis() > 10 { 1 } else { 245 };
                        let timings_fmt = cfg
                            .timings
                            .then(|| format!(" ({}, run: {:>9.3?})", compile_timing_fmt, run_end))
                            .unwrap_or_default();
                        Colour::Fixed(timings_color).paint(timings_fmt).to_string()
                    };

                    process_result(result, &mut test, cfg, timings)
                } else {
                    println!("{} (diagnostics)", Colour::Red.bold().paint("FAILED"));
                    if cfg.verbose {
                        let formatter = Formatter::new(&test.source, warnings);
                        println!("{formatter}");
                    }
                    // mark as failure, did not expect any warnings
                    true
                }
            }
            Err(diagnostics) => {
                warnings_count += diagnostics.warnings().len();
                process_compilation_diagnostics(&test, cfg, diagnostics, compile_timing_fmt)
            }
        };
        if failed {
            failed_count += 1;
        }
    }

    print_result(total_count, failed_count, warnings_count);
}

fn sanitize_lines(input: String) -> String {
    input
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.trim())
        .collect::<Vec<&str>>()
        .join("\n")
}

fn process_result(
    result: Result<Value, Terminate>,
    test: &mut Test,
    config: &TestConfig,
    timings: String,
) -> bool {
    if test.skip {
        return false;
    }

    match result {
        Ok(got) => {
            let got_value = vrl_value_to_json_value(got);
            let mut failed = false;

            let want = test.result.clone();
            let want_value = if want.starts_with("r'") && want.ends_with('\'') {
                match regex::Regex::new(&want[2..want.len() - 1].replace("\\'", "'")) {
                    Ok(regex) => regex.to_string().into(),
                    Err(_) => want.into(),
                }
            } else if want.starts_with("t'") && want.ends_with('\'') {
                match DateTime::<Utc>::from_str(&want[2..want.len() - 1]) {
                    Ok(dt) => dt.to_rfc3339_opts(SecondsFormat::AutoSi, true).into(),
                    Err(_) => want.into(),
                }
            } else if want.starts_with("s'") && want.ends_with('\'') {
                want[2..want.len() - 1].into()
            } else {
                serde_json::from_str::<'_, serde_json::Value>(want.trim()).unwrap_or_else(|err| {
                    eprintln!("{}", err);
                    want.into()
                })
            };

            if got_value == want_value {
                print!("{timings}{}", Colour::Green.bold().paint("OK"));
            } else {
                print!("{} (expectation)", Colour::Red.bold().paint("FAILED"));

                if !config.no_diff {
                    let want = serde_json::to_string_pretty(&want_value).unwrap();
                    let got = serde_json::to_string_pretty(&got_value).unwrap();

                    let diff = prettydiff::diff_lines(&want, &got);
                    println!("  {}", diff);
                }

                failed = true;
            }
            println!();

            if config.verbose {
                println!("{:#}", got_value);
            }

            if failed && config.fail_early {
                std::process::exit(1)
            }
            failed
        }
        Err(err) => {
            let mut failed = false;
            let got = err.to_string().trim().to_owned();
            let want = test.result.clone().trim().to_owned();

            if (test.result_approx && compare_partial_diagnostic(&got, &want)) || got == want {
                println!("{}{}", Colour::Green.bold().paint("OK"), timings);
            } else if matches!(err, Terminate::Abort { .. }) {
                let want =
                    serde_json::from_str::<'_, serde_json::Value>(&want).unwrap_or_else(|err| {
                        eprintln!("{}", err);
                        want.into()
                    });

                let got = vrl_value_to_json_value(test.object.clone());
                if got == want {
                    println!("{}{}", Colour::Green.bold().paint("OK"), timings);
                } else {
                    println!("{} (abort)", Colour::Red.bold().paint("FAILED"));

                    if !config.no_diff {
                        let want = serde_json::to_string_pretty(&want).unwrap();
                        let got = serde_json::to_string_pretty(&got).unwrap();
                        let diff = prettydiff::diff_lines(&want, &got);
                        println!("{}", diff);
                    }

                    failed = true;
                }
            } else {
                println!("{} (runtime)", Colour::Red.bold().paint("FAILED"));

                if !config.no_diff {
                    let diff = prettydiff::diff_lines(&want, &got);
                    println!("{}", diff);
                }

                failed = true;
            }

            if config.verbose {
                println!("{:#}", err);
            }

            if failed && config.fail_early {
                std::process::exit(1)
            }
            failed
        }
    }
}

fn process_compilation_diagnostics(
    test: &Test,
    cfg: &TestConfig,
    diagnostics: DiagnosticList,
    compile_timing_fmt: String,
) -> bool {
    let mut failed = false;

    let mut formatter = Formatter::new(&test.source, diagnostics);
    let got = sanitize_lines(formatter.to_string());
    let want = sanitize_lines(test.result.clone());
    if (test.result_approx && compare_partial_diagnostic(&got, &want)) || got == want {
        let timings = {
            let timings_fmt = cfg
                .timings
                .then(|| format!(" ({})", compile_timing_fmt))
                .unwrap_or_default();
            Colour::Fixed(245).paint(timings_fmt).to_string()
        };
        println!("{}{timings}", Colour::Green.bold().paint("OK"));
    } else {
        println!("{} (compilation)", Colour::Red.bold().paint("FAILED"));

        if !cfg.no_diff {
            let diff = prettydiff::diff_lines(&want, &got);
            println!("{}", diff);
        }

        failed = true;
    }

    if cfg.verbose {
        formatter.enable_colors(true);
        println!("{:#}", formatter);
    }

    if failed && cfg.fail_early {
        std::process::exit(1)
    }
    failed
}

fn print_result(total_count: usize, failed_count: usize, warnings_count: usize) {
    let code = i32::from(failed_count > 0);

    println!("\n");

    let passed_count = total_count - failed_count;
    if failed_count > 0 {
        println!(
            "Overall result: {}\n\n  Number failed: {}\n  Number passed: {}",
            Colour::Red.bold().paint("FAILED"),
            Colour::Red.bold().paint(failed_count.to_string()),
            Colour::Green.bold().paint(passed_count.to_string())
        );
    } else {
        println!(
            "Overall result: {}\n  Number passed: {}",
            Colour::Green.bold().paint("SUCCESS"),
            Colour::Green.bold().paint(passed_count.to_string())
        );
    }
    println!(
        "  Number warnings: {}",
        Colour::Yellow.bold().paint(warnings_count.to_string())
    );

    std::process::exit(code)
}

fn compare_partial_diagnostic(got: &str, want: &str) -> bool {
    got.lines()
        .filter(|line| line.trim().starts_with("error[E"))
        .zip(want.trim().lines())
        .all(|(got, want)| got.contains(want))
}

fn vrl_value_to_json_value(value: Value) -> serde_json::Value {
    use serde_json::Value::*;

    match value {
        v @ Value::Bytes(_) => String(v.try_bytes_utf8_lossy().unwrap().into_owned()),
        Value::Integer(v) => v.into(),
        Value::Float(v) => v.into_inner().into(),
        Value::Boolean(v) => v.into(),
        Value::Object(v) => v
            .into_iter()
            .map(|(k, v)| (k, vrl_value_to_json_value(v)))
            .collect::<serde_json::Value>(),
        Value::Array(v) => v
            .into_iter()
            .map(vrl_value_to_json_value)
            .collect::<serde_json::Value>(),
        Value::Timestamp(v) => v.to_rfc3339_opts(SecondsFormat::AutoSi, true).into(),
        Value::Regex(v) => v.to_string().into(),
        Value::Null => Null,
    }
}

fn run_vrl(
    program: Program,
    test_object: &mut Value,
    timezone: TimeZone,
    vrl_runtime: VrlRuntime,
) -> Result<Value, Terminate> {
    let mut metadata = Value::from(BTreeMap::new());
    let mut target = TargetValueRef {
        value: test_object,
        metadata: &mut metadata,
        secrets: &mut Secrets::new(),
    };

    // Insert a dummy secret for examples to use
    target.insert_secret("my_secret", "secret value");
    target.insert_secret("datadog_api_key", "secret value");

    match vrl_runtime {
        VrlRuntime::Ast => {
            // test_enrichment.finish_load();
            let mut runtime = Runtime::new(RuntimeState::default());
            runtime.resolve(&mut target, &program, &timezone)
        }
    }
}
