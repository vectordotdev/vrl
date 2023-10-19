#[macro_use]
extern crate afl;

use std::collections::BTreeMap;

use vrl::compiler::state::RuntimeState;
use vrl::compiler::CompileConfig;
use vrl::compiler::TargetValue;
use vrl::prelude::state::ExternalEnv;
use vrl::prelude::*;
use vrl::value;
use vrl::value::Kind;
use vrl::value::Secrets;

fn main() {
    fuzz!(|data: &[u8]| {
        if let Ok(src) = std::str::from_utf8(data) {
            if src.contains('\\') {
                // skipping known issues with invalid escapes
                return;
            }
            if src.len() > 10_000 {
                // skipping known issues with highly nested expressions
                return;
            }
            fuzz(src);
        }
    });
}

fn fuzz(src: &str) {
    let fns = vrl::stdlib::all();
    let external = ExternalEnv::new_with_kind(Kind::any_object(), Kind::any_object());

    let config = CompileConfig::default();

    if let Ok(result) = vrl::compiler::compile_with_external(src, &fns, &external, config) {
        let mut target = TargetValue {
            value: value!({}),
            metadata: Value::Object(BTreeMap::new()),
            secrets: Secrets::default(),
        };

        let mut state = RuntimeState::default();
        let timezone = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &timezone);
        if let Ok(_value) = result.program.resolve(&mut ctx) {
            let type_info = result.program.final_type_info();
            let expected_kind = type_info.state.external.target_kind();
            let actual_kind = Kind::from(target.value);
            if let Err(path) = expected_kind.is_superset(&actual_kind) {
                panic!("Value doesn't match at path: '{}'\n\nType at path = {:?}\n\nDefinition at path = {:?}",
                    path,
                    actual_kind.at_path(&path).debug_info(),
                    expected_kind.at_path(&path).debug_info());
            }
            // TODO: check metadata type, result type, fallibility, abortability
        }
    }
}
