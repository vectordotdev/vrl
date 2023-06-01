use std::collections::BTreeMap;
use vrl::{
    compiler::{state::RuntimeState, Context, TargetValue, TimeZone},
    value,
    value::{Secrets, Value},
};

fn main() {
    // The VRL source code to run. This just retrieves the value of field "x"
    let src = ".x";

    // Use all of the std library functions
    let fns = vrl::stdlib::all();

    // Compile the program (and panic if it's invalid)
    let result = vrl::compiler::compile(src, &fns).unwrap();

    // This is the target that can be accessed / modified in the VRL program.
    // You can use any custom type that implements `Target`, but `TargetValue` is also provided for convenience.
    let mut target = TargetValue {
        // the value starts as just an object with a single field "x" set to 1
        value: value!({x: 1}),
        // the metadata is empty
        metadata: Value::Object(BTreeMap::new()),
        // and there are no secrets associated with the target
        secrets: Secrets::default(),
    };

    // The current state of the runtime (i.e. local variables)
    let mut state = RuntimeState::default();

    let timezone = TimeZone::default();

    // A context bundles all the info necessary for the runtime to resolve a value.
    let mut ctx = Context::new(&mut target, &mut state, &timezone);

    // This executes the VRL program, making any modifications to the target, and returning a result.
    let value = result.program.resolve(&mut ctx).unwrap();

    assert_eq!(value, value!(1));
}
