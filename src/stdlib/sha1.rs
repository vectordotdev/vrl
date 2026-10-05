use super::util::hex_encode;
use crate::compiler::prelude::*;
use ::sha1::Digest;

fn sha1_hex(value: &[u8]) -> bytestring::ByteString {
    hex_encode::<40>(sha1::Sha1::digest(value))
}

fn sha1(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    Ok(Value::String(sha1_hex(&value)))
}

#[derive(Clone, Copy, Debug)]
pub struct Sha1;

impl Function for Sha1 {
    fn identifier(&self) -> &'static str {
        "sha1"
    }

    fn usage(&self) -> &'static str {
        "Calculates a [SHA-1](https://en.wikipedia.org/wiki/SHA-1) hash of the `value`."
    }

    fn category(&self) -> &'static str {
        Category::Cryptography.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::BYTES,
            "The string to calculate the hash for.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Calculate sha1 hash",
            source: r#"sha1("foo")"#,
            result: Ok("0beec7b5ea3f0fdbc95d0dd47f3c5bc275da8a33"),
        }]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        if let Some(val) = value.resolve_constant(state)
            && let Ok(bytes) = val.try_bytes()
        {
            Ok(Box::new(crate::compiler::expression::Literal::from(
                sha1_hex(&bytes),
            )))
        } else {
            Ok(Sha1Fn { value }.as_expr())
        }
    }
}

#[derive(Debug, Clone)]
struct Sha1Fn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for Sha1Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        sha1(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        sha1 => Sha1;

        sha {
             args: func_args![value: "foo"],
             want: Ok("0beec7b5ea3f0fdbc95d0dd47f3c5bc275da8a33"),
             tdef: TypeDef::bytes().infallible(),
         }

        sha1_empty {
            args: func_args![value: ""],
            want: Ok("da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            tdef: TypeDef::bytes().infallible(),
        }

        sha1_sentence {
            args: func_args![value: "The quick brown fox jumps over the lazy dog"],
            want: Ok("2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];

    #[test]
    fn test_sha1_compiles_to_literal() {
        use crate::compiler::CompileConfig;

        let state = state::TypeState::default();
        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let mut args = ArgumentList::default();
        args.insert("value", Value::from("foo").into());

        let expr = Sha1.compile(&state, &mut ctx, args).unwrap();
        assert_eq!(
            expr.resolve_constant(&state),
            Some(Value::from("0beec7b5ea3f0fdbc95d0dd47f3c5bc275da8a33"))
        );
    }

    #[test]
    fn test_sha1_compiles_dynamic() {
        use crate::compiler::CompileConfig;
        use crate::compiler::expression::Variable;
        use crate::compiler::parser::Ident;

        let mut state = state::TypeState::default();
        state.local.insert_variable(
            Ident::new("foo"),
            type_def::Details {
                type_def: TypeDef::bytes(),
                value: None,
            },
        );

        let mut ctx = FunctionCompileContext::new(Span::default(), CompileConfig::default());
        let var = Variable::new((0, 0).into(), Ident::new("foo"), &state.local).unwrap();

        let mut args = ArgumentList::default();
        args.insert("value", var.into());

        let expr = Sha1.compile(&state, &mut ctx, args).unwrap();
        assert!(expr.resolve_constant(&state).is_none());
    }
}
