#![deny(warnings)]
#![allow(clippy::print_stdout)] // tests
#![allow(clippy::print_stderr)] // tests

#[allow(clippy::module_inception)]
mod test;

use crate::compiler::{
    compile_with_external,
    runtime::{Runtime, Terminate},
    state::{ExternalEnv, RuntimeState},
    value::VrlValueConvert,
    CompilationResult, CompileConfig, Function, Program, SecretTarget, TargetValueRef, TimeZone,
    VrlRuntime,
};
use crate::diagnostic::Formatter;
pub use test::Test;

use std::path::{PathBuf, MAIN_SEPARATOR};
use std::{collections::BTreeMap, env, str::FromStr, time::Instant};

use crate::value::Secrets;
use crate::value::Value;
use ansi_term::Colour;
use chrono::{DateTime, SecondsFormat, Utc};

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
    let mut category = "".to_owned();

    for mut test in tests {
        if category != test.category {
            category = test.category.clone();
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
        }

        let state = RuntimeState::default();
        let runtime = Runtime::new(state);

        let external_env = ExternalEnv::default();
        let (mut config, config_metadata) = (compile_config_provider)();

        // Set some read-only paths that can be tested
        for (path, recursive) in &test.read_only_paths {
            config.set_read_only_path(path.clone(), *recursive);
        }

        let compile_start = Instant::now();
        let result = compile_with_external(&test.source, functions, &external_env, config);
        let compile_end = compile_start.elapsed();

        let want = test.result.clone();
        let timezone = cfg.timezone;

        let compile_timing_fmt = cfg
            .timings
            .then(|| format!("comp: {:>9.3?}", compile_end))
            .unwrap_or_default();

        match result {
            Ok(CompilationResult {
                program,
                warnings,
                config: _,
            }) if warnings.is_empty() => {
                let run_start = Instant::now();
                finalize_config(config_metadata);
                let result = run_vrl(runtime, program, &mut test, timezone, cfg.runtime);
                let run_end = run_start.elapsed();

                let timings_fmt = cfg
                    .timings
                    .then(|| format!(" ({}, run: {:>9.3?})", compile_timing_fmt, run_end))
                    .unwrap_or_default();

                let timings_color = if run_end.as_millis() > 10 { 1 } else { 245 };
                let timings = Colour::Fixed(timings_color).paint(timings_fmt);

                match result {
                    Ok(got) => {
                        let got = vrl_value_to_json_value(got);
                        let mut failed = false;

                        if !test.skip {
                            let want = if want.starts_with("r'") && want.ends_with('\'') {
                                match regex::Regex::new(
                                    &want[2..want.len() - 1].replace("\\'", "'"),
                                ) {
                                    Ok(want) => want.to_string().into(),
                                    Err(_) => want.into(),
                                }
                            } else if want.starts_with("t'") && want.ends_with('\'') {
                                match DateTime::<Utc>::from_str(&want[2..want.len() - 1]) {
                                    Ok(want) => {
                                        want.to_rfc3339_opts(SecondsFormat::AutoSi, true).into()
                                    }
                                    Err(_) => want.into(),
                                }
                            } else if want.starts_with("s'") && want.ends_with('\'') {
                                want[2..want.len() - 1].into()
                            } else {
                                match serde_json::from_str::<'_, serde_json::Value>(want.trim()) {
                                    Ok(want) => want,
                                    Err(err) => {
                                        eprintln!("{}", err);
                                        want.into()
                                    }
                                }
                            };
                            if got == want {
                                print!("{}{}", Colour::Green.bold().paint("OK"), timings,);
                            } else {
                                print!("{} (expectation)", Colour::Red.bold().paint("FAILED"));
                                failed_count += 1;

                                if !cfg.no_diff {
                                    let want = serde_json::to_string_pretty(&want).unwrap();
                                    let got = serde_json::to_string_pretty(&got).unwrap();

                                    let diff = prettydiff::diff_lines(&want, &got);
                                    println!("  {}", diff);
                                }

                                failed = true;
                            }

                            println!();
                        }

                        if cfg.verbose {
                            println!("{:#}", got);
                        }

                        if failed && cfg.fail_early {
                            std::process::exit(1)
                        }
                    }
                    Err(err) => {
                        let mut failed = false;
                        if !test.skip {
                            let got = err.to_string().trim().to_owned();
                            let want = want.trim().to_owned();

                            if (test.result_approx && compare_partial_diagnostic(&got, &want))
                                || got == want
                            {
                                println!("{}{}", Colour::Green.bold().paint("OK"), timings);
                            } else if matches!(err, Terminate::Abort { .. }) {
                                let want =
                                    match serde_json::from_str::<'_, serde_json::Value>(&want) {
                                        Ok(want) => want,
                                        Err(err) => {
                                            eprintln!("{}", err);
                                            want.into()
                                        }
                                    };

                                let got = vrl_value_to_json_value(test.object.clone());
                                if got == want {
                                    println!("{}{}", Colour::Green.bold().paint("OK"), timings);
                                } else {
                                    println!("{} (abort)", Colour::Red.bold().paint("FAILED"));
                                    failed_count += 1;

                                    if !cfg.no_diff {
                                        let want = serde_json::to_string_pretty(&want).unwrap();
                                        let got = serde_json::to_string_pretty(&got).unwrap();
                                        let diff = prettydiff::diff_lines(&want, &got);
                                        println!("{}", diff);
                                    }

                                    failed = true;
                                }
                            } else {
                                println!("{} (runtime)", Colour::Red.bold().paint("FAILED"));
                                failed_count += 1;

                                if !cfg.no_diff {
                                    let diff = prettydiff::diff_lines(&want, &got);
                                    println!("{}", diff);
                                }

                                failed = true;
                            }
                        }

                        if cfg.verbose {
                            println!("{:#}", err);
                        }

                        if failed && cfg.fail_early {
                            std::process::exit(1)
                        }
                    }
                }
            }
            Ok(CompilationResult {
                program: _,
                warnings: diagnostics,
                config: _,
            })
            | Err(diagnostics) => {
                let mut failed = false;
                let mut formatter = Formatter::new(&test.source, diagnostics);
                if !test.skip {
                    let got = formatter.to_string().trim().to_owned();
                    let want = want.trim().to_owned();

                    if (test.result_approx && compare_partial_diagnostic(&got, &want))
                        || got == want
                    {
                        let timings_fmt = cfg
                            .timings
                            .then(|| format!(" ({})", compile_timing_fmt))
                            .unwrap_or_default();
                        let timings = Colour::Fixed(245).paint(timings_fmt);

                        println!("{}{}", Colour::Green.bold().paint("OK"), timings);
                    } else {
                        println!("{} (compilation)", Colour::Red.bold().paint("FAILED"));
                        failed_count += 1;

                        if !cfg.no_diff {
                            let diff = prettydiff::diff_lines(&want, &got);
                            println!("{}", diff);
                        }

                        failed = true;
                    }
                }

                if cfg.verbose {
                    formatter.enable_colors(true);
                    println!("{:#}", formatter);
                }

                if failed && cfg.fail_early {
                    std::process::exit(1)
                }
            }
        }
    }
    print_result(total_count, failed_count)
}

fn print_result(total_count: usize, failed_count: usize) {
    let code = i32::from(failed_count > 0);

    println!("\n");

    if failed_count > 0 {
        println!(
            "Overall result: {}\n\n  Number failed: {}\n  Number passed: {}",
            Colour::Red.bold().paint("FAILED"),
            failed_count,
            total_count - failed_count
        );
    } else {
        println!(
            "Overall result: {}\n  Number passed: {total_count}",
            Colour::Green.bold().paint("SUCCESS")
        );
    }

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

#[allow(clippy::too_many_arguments)]
fn run_vrl(
    mut runtime: Runtime,
    program: Program,
    test: &mut Test,
    timezone: TimeZone,
    vrl_runtime: VrlRuntime,
) -> Result<Value, Terminate> {
    let mut metadata = Value::from(BTreeMap::new());
    let mut target = TargetValueRef {
        value: &mut test.object,
        metadata: &mut metadata,
        secrets: &mut Secrets::new(),
    };

    // Insert a dummy secret for examples to use
    target.insert_secret("my_secret", "secret value");
    target.insert_secret("datadog_api_key", "secret value");

    match vrl_runtime {
        VrlRuntime::Ast => {
            // test_enrichment.finish_load();
            runtime.resolve(&mut target, &program, &timezone)
        }
    }
}
