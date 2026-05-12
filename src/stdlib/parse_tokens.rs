use crate::compiler::prelude::*;
use crate::core::tokenize;

fn parse_tokens(value: &Value) -> Resolved {
    let string = value.try_bytes_utf8_lossy()?;
    let tokens: Value = tokenize::parse(&string)
        .into_iter()
        .map(|token| match token {
            "" | "-" => Value::Null,
            _ => token.to_owned().into(),
        })
        .collect::<Vec<_>>()
        .into();
    Ok(tokens)
}

#[derive(Clone, Copy, Debug)]
pub struct ParseTokens;

impl Function for ParseTokens {
    fn identifier(&self) -> &'static str {
        "parse_tokens"
    }

    fn usage(&self) -> &'static str {
        indoc! {r#"
            Parses the `value` in token format. A token is considered to be one of the following:

            * A word surrounded by whitespace.
            * Text delimited by double quotes: `".."`. Quotes can be included in the token if they are escaped by a backslash (`\`).
            * Text delimited by square brackets: `[..]`. Closing square brackets can be included in the token if they are escaped by a backslash (`\`).
        "#}
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a properly formatted tokenized string."]
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY
    }

    fn notices(&self) -> &'static [&'static str] {
        &[indoc! {"
            All token values are returned as strings. We recommend manually coercing values to
            desired types as you see fit.
        "}]
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Parse tokens",
            source: r#"parse_tokens(s'A sentence "with \"a\" sentence inside" and [some brackets]')"#,
            result: Ok(
                r#"["A", "sentence", "with \\\"a\\\" sentence inside", "and", "some brackets"]"#,
            ),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(ParseTokensFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::BYTES,
            "The string to tokenize.",
        )];
        PARAMETERS
    }
}

#[derive(Debug, Clone)]
struct ParseTokensFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseTokensFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_tokens(&value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::array(Collection::from_unknown(Kind::bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        parse_tokens => ParseTokens;

        parses {
            args: func_args![value: "217.250.207.207 - - [07/Sep/2020:16:38:00 -0400] \"DELETE /deliverables/next-generation/user-centric HTTP/1.1\" 205 11881"],
            want: Ok(vec![
                            "217.250.207.207".into(),
                            Value::Null,
                            Value::Null,
                            "07/Sep/2020:16:38:00 -0400".into(),
                            "DELETE /deliverables/next-generation/user-centric HTTP/1.1".into(),
                            "205".into(),
                            "11881".into(),

                    ]),
            tdef: TypeDef::array(Collection::from_unknown(Kind::bytes())),
        }
    ];
}
