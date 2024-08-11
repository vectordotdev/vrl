use std::collections::BTreeMap;

use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct ParseInfluxDB;

impl Function for ParseInfluxDB {
    fn identifier(&self) -> &'static str {
        "parse_influxdb"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(ParseInfluxDBFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        todo!()
    }
}

#[derive(Clone, Debug)]
struct ParseInfluxDBFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseInfluxDBFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        todo!()
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        // TODO: what is the correct type?
        TypeDef::object(inner_kind()).fallible()
    }
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        ("level".into(), Kind::bytes()),
        ("timestamp".into(), Kind::timestamp()),
        ("id".into(), Kind::integer()),
        ("file".into(), Kind::bytes()),
        ("line".into(), Kind::integer()),
        ("message".into(), Kind::bytes()),
    ])
}
