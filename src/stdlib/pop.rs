use crate::compiler::prelude::*;

fn pop(value: Value) -> Resolved {
    let mut value = value.try_array()?;
    value.pop();
    Ok(value.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Pop;

impl Function for Pop {
    fn identifier(&self) -> &'static str {
        "pop"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ARRAY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "pop array",
            source: "pop(value: [0, 1])",
            result: Ok("[0]"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(PopFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct PopFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for PopFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        pop(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        self.value
            .type_def(state)
            .fallible_unless(Kind::array(Collection::any()))
            .restrict_array()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{btreemap, value};

    test_function![
        pop => Pop;

        empty_array {
            args: func_args![value: value!([])],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::empty()),
        }

        null_array {
            args: func_args![value: value!(null)],
            want: Err("expected array, got null"),
            tdef: TypeDef::array(Collection::any()).fallible(),
        }

        mixed_array_types {
            args: func_args![value: value!([1, 2, 3, true, 5.0, "bar"])],
            want: Ok(value!([1, 2, 3, true, 5.0])),
            tdef: TypeDef::array(btreemap! {
                Index::from(0) => Kind::integer(),
                Index::from(1) => Kind::integer(),
                Index::from(2) => Kind::integer(),
                Index::from(3) => Kind::boolean(),
                Index::from(4) => Kind::float(),
                Index::from(5) => Kind::bytes(),
            }),
        }

        integer_array {
            args: func_args![value: value!([0, 1, 2, 3])],
            want: Ok(value!([0, 1, 2])),
            tdef: TypeDef::array(btreemap! {
                Index::from(0) => Kind::integer(),
                Index::from(1) => Kind::integer(),
                Index::from(2) => Kind::integer(),
                Index::from(3) => Kind::integer(),
            }),
        }

    ];
}
