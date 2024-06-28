use crate::compiler::prelude::*;
use crate::parsing::query_string::parse_query_string;

#[derive(Clone, Copy, Debug)]
pub struct ParseQueryString;

impl Function for ParseQueryString {
    fn identifier(&self) -> &'static str {
        "parse_query_string"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "parse query string",
            source: r#"parse_query_string("foo=1&bar=2")"#,
            result: Ok(r#"
                {
                    "foo": "1",
                    "bar": "2"
                }
            "#),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(ParseQueryStringFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }
}

#[derive(Debug, Clone)]
struct ParseQueryStringFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseQueryStringFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.value.resolve(ctx)?.try_bytes()?;
        parse_query_string(&bytes, false)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind())
    }
}

fn inner_kind() -> Collection<Field> {
    Collection::from_unknown(Kind::bytes().or_array(Collection::any()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        parse_query_string => ParseQueryString;

        complete {
            args: func_args![value: value!("foo=%2B1&bar=2&xyz=&abc")],
            want: Ok(value!({
                foo: "+1",
                bar: "2",
                xyz: "",
                abc: "",
            })),
            tdef: TypeDef::object(inner_kind()),
        }

        multiple_values {
            args: func_args![value: value!("foo=bar&foo=xyz")],
            want: Ok(value!({
                foo: ["bar", "xyz"],
            })),
            tdef: TypeDef::object(inner_kind()),
        }

        ruby_on_rails_multiple_values {
            args: func_args![value: value!("?foo%5b%5d=bar&foo%5b%5d=xyz")],
            want: Ok(value!({
                "foo[]": ["bar", "xyz"],
            })),
            tdef: TypeDef::object(inner_kind()),
        }

        empty_key {
            args: func_args![value: value!("=&=")],
            want: Ok(value!({
                "": ["", ""],
            })),
            tdef: TypeDef::object(inner_kind()),
        }

        single_key {
            args: func_args![value: value!("foo")],
            want: Ok(value!({
                foo: "",
            })),
            tdef: TypeDef::object(inner_kind()),
        }

        empty {
            args: func_args![value: value!("")],
            want: Ok(value!({})),
            tdef: TypeDef::object(inner_kind()),
        }

        starts_with_question_mark {
            args: func_args![value: value!("?foo=bar")],
            want: Ok(value!({
                foo: "bar",
            })),
            tdef: TypeDef::object(inner_kind()),
        }
    ];
}
