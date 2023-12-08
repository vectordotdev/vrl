use crate::compiler::TargetValue;
use std::borrow::Cow::{self, Borrowed, Owned};
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::compiler::runtime::Runtime;
use crate::compiler::state::{RuntimeState, TypeState};
use crate::compiler::TimeZone;
use crate::compiler::{compile_with_state, CompileConfig, Function, Program, Target, VrlRuntime};
use crate::diagnostic::Formatter;
use crate::owned_value_path;
use crate::path::OwnedTargetPath;
use crate::value::Secrets;
use crate::value::Value;
use indoc::indoc;
use once_cell::sync::Lazy;
use prettytable::{format, Cell, Row, Table};
use regex::Regex;
use rustyline::{
    completion::Completer,
    error::ReadlineError,
    highlight::{Highlighter, MatchingBracketHighlighter},
    hint::{Hinter, HistoryHinter},
    history::MemHistory,
    validate::{self, ValidationResult, Validator},
    Context, Editor, Helper,
};

// Create a list of all possible error values for potential docs lookup
static ERRORS: Lazy<Vec<String>> = Lazy::new(|| {
    [
        100, 101, 102, 103, 104, 105, 106, 107, 108, 110, 203, 204, 205, 206, 207, 208, 209, 300,
        301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 400, 401, 402, 403,
        601, 620, 630, 640, 650, 651, 652, 660, 701,
    ]
    .iter()
    .map(std::string::ToString::to_string)
    .collect()
});

const DOCS_URL: &str = "https://vector.dev/docs/reference/vrl";
const ERRORS_URL_ROOT: &str = "https://errors.vrl.dev";
const RESERVED_TERMS: &[&str] = &[
    "next",
    "prev",
    "exit",
    "quit",
    "help",
    "help functions",
    "help funcs",
    "help fs",
    "help docs",
];

pub(crate) fn run(
    mut objects: Vec<TargetValue>,
    timezone: TimeZone,
    vrl_runtime: VrlRuntime,
    stdlib_functions: Vec<Box<dyn Function>>,
) -> Result<(), rustyline::error::ReadlineError> {
    let stdlib_functions = Rc::new(stdlib_functions);
    let mut index = 0;
    let func_docs_regex = Regex::new(r"^help\sdocs\s(\w{1,})$").unwrap();
    let error_docs_regex = Regex::new(r"^help\serror\s(\w{1,})$").unwrap();

    let mut state = TypeState::default();

    let mut rt = Runtime::new(RuntimeState::default());
    let mut rl = Editor::<Repl, MemHistory>::new()?;
    rl.set_helper(Some(Repl::new(stdlib_functions.clone())));

    #[allow(clippy::print_stdout)]
    {
        println!("{BANNER_TEXT}");
    }

    loop {
        let readline = rl.readline("$ ");
        match readline.as_deref() {
            Ok(line) if line == "exit" || line == "quit" => break,
            Ok(line) if line == "help" => print_help_text(),
            Ok(line) if line == "help functions" || line == "help funcs" || line == "help fs" => {
                print_function_list()
            }
            Ok(line) if line == "help docs" => open_url(DOCS_URL),
            // Capture "help error <code>"
            Ok(line) if error_docs_regex.is_match(line) => show_error_docs(line, &error_docs_regex),
            // Capture "help docs <func_name>"
            Ok(line) if func_docs_regex.is_match(line) => show_func_docs(line, &func_docs_regex),
            Ok(line) => {
                rl.add_history_entry(line)?;

                let command = match line {
                    "next" => {
                        // allow adding one new object at a time
                        if index < objects.len()
                            && objects.last().map(|x| &x.value) != Some(&Value::Null)
                        {
                            index = index.saturating_add(1);
                        }

                        // add new object
                        if index == objects.len() {
                            objects.push(TargetValue {
                                value: Value::Null,
                                metadata: Value::Object(BTreeMap::new()),
                                secrets: Secrets::new(),
                            });
                        }

                        "."
                    }
                    "prev" => {
                        index = index.saturating_sub(1);

                        // remove empty last object
                        if objects.last().map(|x| &x.value) == Some(&Value::Null) {
                            let _last = objects.pop();
                        }

                        "."
                    }
                    "" => continue,
                    _ => line,
                };

                let result = resolve(
                    objects.get_mut(index).expect("object should exist"),
                    &mut rt,
                    command,
                    &mut state,
                    timezone,
                    vrl_runtime,
                    &stdlib_functions,
                );

                let string = match result {
                    Ok(v) => v.to_string(),
                    Err(v) => v.to_string(),
                };

                #[allow(clippy::print_stdout)]
                {
                    println!("{string}\n");
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(err) => {
                #[allow(clippy::print_stdout)]
                {
                    println!("unable to read line: {err}");
                }
                break;
            }
        }
    }
    Ok(())
}

fn resolve(
    target: &mut TargetValue,
    runtime: &mut Runtime,
    program: &str,
    state: &mut TypeState,
    timezone: TimeZone,
    vrl_runtime: VrlRuntime,
    stdlib_functions: &[Box<dyn Function>],
) -> Result<Value, String> {
    let mut config = CompileConfig::default();
    // The CLI should be moved out of the "vrl" module, and then it can use the `vector-core::compile_vrl` function which includes this automatically
    config.set_read_only_path(OwnedTargetPath::metadata(owned_value_path!("vector")), true);

    let program = match compile_with_state(program, stdlib_functions, state, config) {
        Ok(result) => result.program,
        Err(diagnostics) => {
            return Err(Formatter::new(program, diagnostics).colored().to_string());
        }
    };

    *state = program.final_type_info().state;
    execute(runtime, &program, target, timezone, vrl_runtime)
}

fn execute(
    runtime: &mut Runtime,
    program: &Program,
    object: &mut dyn Target,
    timezone: TimeZone,
    vrl_runtime: VrlRuntime,
) -> Result<Value, String> {
    match vrl_runtime {
        VrlRuntime::Ast => runtime
            .resolve(object, program, &timezone)
            .map_err(|err| err.to_string()),
    }
}

struct Repl {
    highlighter: MatchingBracketHighlighter,
    history_hinter: HistoryHinter,
    colored_prompt: String,
    hints: Vec<&'static str>,
    stdlib_functions: Rc<Vec<Box<dyn Function>>>,
}

impl Repl {
    fn new(stdlib_functions: Rc<Vec<Box<dyn Function>>>) -> Self {
        Self {
            highlighter: MatchingBracketHighlighter::new(),
            history_hinter: HistoryHinter {},
            colored_prompt: "$ ".to_owned(),
            hints: initial_hints(&stdlib_functions),
            stdlib_functions,
        }
    }
}

fn initial_hints(funcs: &[Box<dyn Function>]) -> Vec<&'static str> {
    funcs
        .iter()
        .map(|f| f.identifier())
        .chain(RESERVED_TERMS.iter().copied())
        .collect()
}

impl Helper for Repl {}
impl Completer for Repl {
    type Candidate = String;
}

impl Hinter for Repl {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }

        let mut hints: Vec<String> = Vec::new();

        // Add all function names to the hints
        let mut func_names = crate::stdlib::all()
            .iter()
            .map(|f| f.identifier().into())
            .collect::<Vec<String>>();

        hints.append(&mut func_names);

        // Check history first
        if let Some(hist) = self.history_hinter.hint(line, pos, ctx) {
            return Some(hist);
        }

        // Then check the other built-in hints
        self.hints.iter().find_map(|hint| {
            if pos > 0 && hint.starts_with(&line[..pos]) {
                Some(String::from(&hint[pos..]))
            } else {
                None
            }
        })
    }
}

impl Highlighter for Repl {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(&self.colored_prompt)
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for Repl {
    fn validate(
        &self,
        ctx: &mut validate::ValidationContext,
    ) -> rustyline::Result<ValidationResult> {
        let timezone = TimeZone::default();
        let mut state = TypeState::default();
        let mut rt = Runtime::new(RuntimeState::default());
        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Object(BTreeMap::new()),
            secrets: Secrets::new(),
        };

        let result = resolve(
            &mut target,
            &mut rt,
            ctx.input(),
            &mut state,
            timezone,
            VrlRuntime::Ast,
            &self.stdlib_functions,
        );

        let result = match result {
            Err(error) => {
                // TODO: Ideally we'd used typed errors for this, but
                // that requires some more work to the VRL compiler.
                if error.contains("syntax error") && error.contains("unexpected end of program") {
                    ValidationResult::Incomplete
                } else {
                    ValidationResult::Valid(None)
                }
            }

            Ok(..) => ValidationResult::Valid(None),
        };

        Ok(result)
    }

    fn validate_while_typing(&self) -> bool {
        false
    }
}

fn print_function_list() {
    let table_format = *format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
    let num_columns = 3;

    let mut func_table = Table::new();
    func_table.set_format(table_format);
    crate::stdlib::all()
        .chunks(num_columns)
        .map(|funcs| {
            // Because it's possible that some chunks are only partial, e.g. have only two Some(_)
            // values when num_columns is 3, this logic below is necessary to avoid panics caused
            // by inappropriately calling funcs.get(_) on a None.
            let mut ids: Vec<Cell> = Vec::new();

            for n in 0..num_columns {
                if let Some(v) = funcs.get(n) {
                    ids.push(Cell::new(v.identifier()));
                }
            }

            func_table.add_row(Row::new(ids));
        })
        .for_each(drop);

    func_table.printstd();
}

fn print_help_text() {
    #[allow(clippy::print_stdout)]
    {
        println!("{HELP_TEXT}");
    }
}

fn open_url(url: &str) {
    if let Err(err) = webbrowser::open(url) {
        #[allow(clippy::print_stdout)]
        {
            println!(
                "couldn't open default web browser: {err}\n\
            you can access the desired documentation at {url}"
            );
        }
    }
}

fn show_func_docs(line: &str, pattern: &Regex) {
    // Unwrap is okay in both cases here, as there's guaranteed to be two matches ("help docs" and
    // "help docs <func_name>")
    let matches = pattern.captures(line).unwrap();
    let func_name = matches.get(1).unwrap().as_str();

    if crate::stdlib::all()
        .iter()
        .any(|f| f.identifier() == func_name)
    {
        let func_url = format!("{DOCS_URL}/functions/#{func_name}");
        open_url(&func_url);
    } else {
        #[allow(clippy::print_stdout)]
        {
            println!("function name {func_name} not recognized");
        }
    }
}

fn show_error_docs(line: &str, pattern: &Regex) {
    // As in show_func_docs, unwrap is okay here
    let matches = pattern.captures(line).unwrap();
    let error_code = matches.get(1).unwrap().as_str();

    if ERRORS.iter().any(|e| e == error_code) {
        let error_code_url = format!("{ERRORS_URL_ROOT}/{error_code}");
        open_url(&error_code_url);
    } else {
        #[allow(clippy::print_stdout)]
        {
            println!("error code {error_code} not recognized");
        }
    }
}

const HELP_TEXT: &str = indoc! {r#"
    VRL REPL commands:
      help functions     Display a list of currently available VRL functions (aliases: ["help funcs", "help fs"])
      help docs          Navigate to the VRL docs on the Vector website
      help docs <func>   Navigate to the VRL docs for the specified function
      help error <code>  Navigate to the docs for a specific error code
      next               Load the next object or create a new one
      prev               Load the previous object
      exit               Terminate the program
"#};

const BANNER_TEXT: &str = indoc! {r#"
    > VVVVVVVV           VVVVVVVVRRRRRRRRRRRRRRRRR   LLLLLLLLLLL
    > V::::::V           V::::::VR::::::::::::::::R  L:::::::::L
    > V::::::V           V::::::VR::::::RRRRRR:::::R L:::::::::L
    > V::::::V           V::::::VRR:::::R     R:::::RLL:::::::LL
    >  V:::::V           V:::::V   R::::R     R:::::R  L:::::L
    >   V:::::V         V:::::V    R::::R     R:::::R  L:::::L
    >    V:::::V       V:::::V     R::::RRRRRR:::::R   L:::::L
    >     V:::::V     V:::::V      R:::::::::::::RR    L:::::L
    >      V:::::V   V:::::V       R::::RRRRRR:::::R   L:::::L
    >       V:::::V V:::::V        R::::R     R:::::R  L:::::L
    >        V:::::V:::::V         R::::R     R:::::R  L:::::L
    >         V:::::::::V          R::::R     R:::::R  L:::::L         LLLLLL
    >          V:::::::V         RR:::::R     R:::::RLL:::::::LLLLLLLLL:::::L
    >           V:::::V          R::::::R     R:::::RL::::::::::::::::::::::L
    >            V:::V           R::::::R     R:::::RL::::::::::::::::::::::L
    >             VVV            RRRRRRRR     RRRRRRRLLLLLLLLLLLLLLLLLLLLLLLL
    >
    >                     VECTOR    REMAP    LANGUAGE
    >
    >
    > Welcome!
    >
    > The CLI is running in REPL (Read-eval-print loop) mode.
    >
    > To run the CLI in regular mode, add a program to your command.
    >
    > VRL REPL commands:
    >   help              Learn more about VRL
    >   next              Load the next object or create a new one
    >   prev              Load the previous object
    >   exit              Terminate the program
    >
    > Any other value is resolved to a VRL expression.
    >
    > Try it out now by typing `.` and hitting [enter] to see the result.
"#};
