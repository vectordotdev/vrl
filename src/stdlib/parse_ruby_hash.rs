use crate::compiler::prelude::*;

fn parse_ruby_hash(value: Value) -> Resolved {
    let input = value.try_bytes_utf8_lossy()?;
    crate::parsing::ruby_hash::parse(&input)
}

#[derive(Clone, Copy, Debug)]
pub struct ParseRubyHash;

impl Function for ParseRubyHash {
    fn identifier(&self) -> &'static str {
        "parse_ruby_hash"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "parse ruby hash",
            source: r#"parse_ruby_hash!(s'{ "test" => "value", "testNum" => 0.2, "testObj" => { "testBool" => true, "testNull" => nil } }')"#,
            result: Ok(r#"
                {
                    "test": "value",
                    "testNum": 0.2,
                    "testObj": {
                        "testBool": true,
                        "testNull": null
                    }
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
        Ok(ParseRubyHashFn { value }.as_expr())
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
struct ParseRubyHashFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseRubyHashFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_ruby_hash(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(Collection::from_unknown(inner_kinds())).fallible()
    }
}

fn inner_kinds() -> Kind {
    Kind::null()
        | Kind::bytes()
        | Kind::float()
        | Kind::boolean()
        | Kind::array(Collection::any())
        | Kind::object(Collection::any())
}
