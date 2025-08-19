use std::collections::BTreeMap;

use crate::compiler::prelude::*;
use crate::value;

use super::util::round_to_precision;

const EARTH_RADIUS_IN_METERS: f64 = 6_371_008.8;
const EARTH_RADIUS_IN_KILOMETERS: f64 = EARTH_RADIUS_IN_METERS / 1000.0;
const EARTH_RADIUS_IN_MILES: f64 = EARTH_RADIUS_IN_KILOMETERS * 0.621_371_2;

fn haversine_distance(
    latitude1: Value,
    longitude1: Value,
    latitude2: Value,
    longitude2: Value,
    measurement_unit: &MeasurementUnit,
) -> Resolved {
    let latitude1 = latitude1.try_float()?.to_radians();
    let longitude1 = longitude1.try_float()?.to_radians();
    let latitude2 = latitude2.try_float()?.to_radians();
    let longitude2 = longitude2.try_float()?.to_radians();

    let mut result = ObjectMap::new();

    // Distance calculation
    let dlon = longitude2 - longitude1;
    let dlat = latitude2 - latitude1;
    let a =
        (dlat / 2.0).sin().powi(2) + latitude1.cos() * latitude2.cos() * (dlon / 2.0).sin().powi(2);
    let distance = 2.0 * a.sqrt().asin();

    result.insert(
        "distance".into(),
        match measurement_unit {
            MeasurementUnit::Kilometers => Value::from_f64_or_zero(round_to_precision(
                distance * EARTH_RADIUS_IN_KILOMETERS,
                7,
                f64::round,
            )),
            MeasurementUnit::Miles => Value::from_f64_or_zero(round_to_precision(
                distance * EARTH_RADIUS_IN_MILES,
                7,
                f64::round,
            )),
        },
    );

    // Bearing calculation
    let y = dlon.sin() * latitude2.cos();
    let x = latitude1.cos() * latitude2.sin() - latitude1.sin() * latitude2.cos() * dlon.cos();
    let bearing = (y.atan2(x).to_degrees() + 360.0) % 360.0;

    result.insert(
        "bearing".into(),
        Value::from_f64_or_zero(round_to_precision(bearing, 3, f64::round)),
    );

    Ok(result.into())
}

fn measurement_systems() -> Vec<Value> {
    vec![value!("kilometers"), value!("miles")]
}

#[derive(Clone, Debug)]
enum MeasurementUnit {
    Kilometers,
    Miles,
}

#[derive(Clone, Copy, Debug)]
pub struct Haversine;

impl Function for Haversine {
    fn identifier(&self) -> &'static str {
        "haversine"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "latitude1",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "longitude1",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "latitude2",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "longitude2",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "measurement_unit",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let latitude1 = arguments.required("latitude1");
        let longitude1 = arguments.required("longitude1");
        let latitude2 = arguments.required("latitude2");
        let longitude2 = arguments.required("longitude2");

        let measurement_unit = match arguments
            .optional_enum("measurement_unit", &measurement_systems(), state)?
            .unwrap_or_else(|| value!("kilometers"))
            .try_bytes()
            .ok()
            .as_deref()
        {
            Some(b"kilometers") => MeasurementUnit::Kilometers,
            Some(b"miles") => MeasurementUnit::Miles,
            _ => return Err(Box::new(ExpressionError::from("invalid measurement unit"))),
        };

        Ok(HaversineFn {
            latitude1,
            longitude1,
            latitude2,
            longitude2,
            measurement_unit,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "haversine",
                source: "haversine(0.0, 0.0, 10.0, 10.0)",
                result: Ok(indoc!(
                    r#"{
                        "distance": 1568.5227233,
                        "bearing": 44.561
                    }"#
                )),
            },
            Example {
                title: "haversine in miles",
                source: r#"haversine(0.0, 0.0, 10.0, 10.0, measurement_unit: "miles")"#,
                result: Ok(indoc!(
                    r#"{
                        "distance": 974.6348468,
                        "bearing": 44.561
                    }"#
                )),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct HaversineFn {
    latitude1: Box<dyn Expression>,
    longitude1: Box<dyn Expression>,
    latitude2: Box<dyn Expression>,
    longitude2: Box<dyn Expression>,
    measurement_unit: MeasurementUnit,
}

impl FunctionExpression for HaversineFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let latitude1 = self.latitude1.resolve(ctx)?;
        let longitude1 = self.longitude1.resolve(ctx)?;
        let latitude2 = self.latitude2.resolve(ctx)?;
        let longitude2 = self.longitude2.resolve(ctx)?;

        haversine_distance(
            latitude1,
            longitude1,
            latitude2,
            longitude2,
            &self.measurement_unit,
        )
    }

    fn type_def(&self, _state: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind()).infallible()
    }
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        (Field::from("distance"), Kind::float()),
        (Field::from("bearing"), Kind::float()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        haversine => Haversine;

        basic_kilometers {
            args: func_args![latitude1: value!(0.0), longitude1: value!(0.0), latitude2: value!(10.0), longitude2: value!(10.0)],
            want: Ok(value!({ "distance": 1_568.522_723_3, "bearing": 44.561 })),
            tdef: TypeDef::object(inner_kind()).infallible(),
        }

        basic_miles {
            args: func_args![latitude1: value!(0.0), longitude1: value!(0.0), latitude2: value!(10.0), longitude2: value!(10.0), measurement_unit: value!("miles")],
            want: Ok(value!({ "distance": 974.634_846_8, "bearing": 44.561 })),
            tdef: TypeDef::object(inner_kind()).infallible(),
        }
    ];
}
