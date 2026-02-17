use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Exists;

impl Function for Exists {
    fn identifier(&self) -> &'static str {
        "exists"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Checks whether the `path` exists for the target.

            This function distinguishes between a missing path
            and a path with a `null` value. A regular path lookup,
            such as `.foo`, cannot distinguish between the two cases
            since it always returns `null` if the path doesn't exist.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Path.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }
    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "field",
            kind: kind::ANY,
            required: true,
            description: "The path of the field to check.",
            default: None,
            enum_variants: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Exists (field)",
                source: indoc! {r#"
                    . = { "field": 1 }
                    exists(.field)
                "#},
                result: Ok("true"),
            },
            example! {
                title: "Exists (array element)",
                source: indoc! {r#"
                    . = { "array": [1, 2, 3] }
                    exists(.array[2])
                "#},
                result: Ok("true"),
            },
            example! {
                title: "Does not exist (field)",
                source: r#"exists({ "foo": "bar"}.baz)"#,
                result: Ok("false"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let query = arguments.required_query("field")?;

        Ok(ExistsFn { query }.as_expr())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ExistsFn {
    query: expression::Query,
}

fn exists(query: &expression::Query, ctx: &mut Context) -> Resolved {
    let path = query.path();

    if let Some(target_path) = query.external_path() {
        return Ok(ctx
            .target_mut()
            .target_get(&target_path)
            .ok()
            .flatten()
            .is_some()
            .into());
    }

    if let Some(ident) = query.variable_ident() {
        return match ctx.state().variable(ident) {
            Some(value) => Ok(value.get(path).is_some().into()),
            None => Ok(false.into()),
        };
    }

    if let Some(expr) = query.expression_target() {
        let value = expr.resolve(ctx)?;

        return Ok(value.get(path).is_some().into());
    }

    Ok(false.into())
}

impl FunctionExpression for ExistsFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        exists(&self.query, ctx)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}
