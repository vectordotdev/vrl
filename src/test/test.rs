use std::{collections::BTreeMap, fs, path::Path};

use crate::compiler::function::Example;
use crate::path::parse_value_path;
use crate::path::OwnedTargetPath;
use crate::test::{example_vrl_path, test_prefix};
use crate::value::Value;

#[derive(Debug)]
pub struct Test {
    pub name: String,
    pub category: String,
    pub error: Option<String>,
    pub source: String,
    pub object: Value,
    pub result: String,
    pub result_approx: bool,
    pub skip: bool,
    pub check_diagnostics: bool,
    // paths set to read-only
    pub read_only_paths: Vec<(OwnedTargetPath, bool)>,
}

enum CaptureMode {
    Result,
    Object,
    None,
    Done,
}

impl Test {
    pub fn from_path(path: &Path) -> Self {
        let name = test_name(path);
        let category = test_category(path);
        let content = fs::read_to_string(path).expect("content");

        let mut source = String::new();
        let mut object = String::new();
        let mut result = String::new();
        let mut result_approx = false;

        let mut read_only_paths = vec![];

        let mut capture_mode = CaptureMode::None;
        for mut line in content.lines() {
            if line.starts_with('#') && !matches!(capture_mode, CaptureMode::Done) {
                line = line.strip_prefix('#').expect("prefix");
                line = line.strip_prefix(' ').unwrap_or(line);

                if line.starts_with("object:") {
                    capture_mode = CaptureMode::Object;
                    line = line.strip_prefix("object:").expect("object").trim_start();
                } else if line.starts_with("result: ~") {
                    capture_mode = CaptureMode::Result;
                    result_approx = true;
                    line = line.strip_prefix("result: ~").expect("result").trim_start();
                } else if line.starts_with("result:") {
                    capture_mode = CaptureMode::Result;
                    line = line.strip_prefix("result:").expect("result").trim_start();
                } else if line.starts_with("read_only:") {
                    let path_str = line.strip_prefix("read_only:").expect("read-only").trim();
                    read_only_paths.push((
                        OwnedTargetPath::event(parse_value_path(path_str).expect("valid path")),
                        false,
                    ));
                    continue;
                } else if line.starts_with("read_only_recursive:") {
                    let path_str = line
                        .strip_prefix("read_only_recursive:")
                        .expect("read-only")
                        .trim();
                    read_only_paths.push((
                        OwnedTargetPath::event(parse_value_path(path_str).expect("valid path")),
                        true,
                    ));
                    continue;
                } else if line.starts_with("read_only_metadata:") {
                    let path_str = line
                        .strip_prefix("read_only_metadata:")
                        .expect("read_only_metadata")
                        .trim();
                    read_only_paths.push((
                        OwnedTargetPath::metadata(parse_value_path(path_str).expect("valid path")),
                        false,
                    ));
                    continue;
                } else if line.starts_with("read_only_metadata_recursive:") {
                    let path_str = line
                        .strip_prefix("read_only_metadata_recursive:")
                        .expect("read-read_only_metadata_recursive")
                        .trim();
                    read_only_paths.push((
                        OwnedTargetPath::metadata(parse_value_path(path_str).expect("valid path")),
                        true,
                    ));
                    continue;
                }

                match capture_mode {
                    CaptureMode::None | CaptureMode::Done => continue,
                    CaptureMode::Result => {
                        result.push_str(line);
                        result.push('\n');
                    }
                    CaptureMode::Object => {
                        object.push_str(line);
                    }
                }
            } else {
                capture_mode = CaptureMode::Done;

                source.push_str(line);
                source.push('\n')
            }
        }

        let mut error = None;
        let object = if object.is_empty() {
            Value::Object(BTreeMap::default())
        } else {
            serde_json::from_str::<'_, Value>(&object).unwrap_or_else(|err| {
                error = Some(format!("unable to parse object as JSON: {}", err));
                Value::Null
            })
        };

        // See https://github.com/rust-lang/rust-clippy/pull/12756
        #[allow(clippy::assigning_clones)]
        {
            result = result.trim_end().to_owned();
        }

        Self {
            name,
            category,
            error,
            source,
            object,
            result,
            result_approx,
            skip: content.starts_with("# SKIP"),
            check_diagnostics: content.starts_with("# DIAGNOSTICS"),
            read_only_paths,
        }
    }

    pub fn from_example(func: impl ToString, example: &Example) -> Self {
        let object = Value::Object(BTreeMap::default());
        let result = match example.result {
            Ok(string) => string.to_owned(),
            Err(err) => err.to_string(),
        };

        Self {
            name: example.title.to_owned(),
            category: format!("functions/{}", func.to_string()),
            error: None,
            source: example.source.to_owned(),
            object,
            result,
            result_approx: false,
            skip: false,
            check_diagnostics: false,
            read_only_paths: vec![],
        }
    }
}

fn test_category(path: &Path) -> String {
    if path == example_vrl_path() {
        return "uncategorized".to_owned();
    }

    let stripped_path = path
        .to_string_lossy()
        .strip_prefix(test_prefix().as_str())
        .expect("test")
        .to_string();

    stripped_path
        .clone()
        .rsplit_once('/')
        .map_or(stripped_path, |x| x.0.to_owned())
}

fn test_name(path: &Path) -> String {
    path.to_string_lossy()
        .rsplit_once('/')
        .unwrap()
        .1
        .trim_end_matches(".vrl")
        .replace('_', " ")
}
