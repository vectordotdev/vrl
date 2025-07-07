use std::collections::BTreeMap;

use crate::compiler::prelude::*;
use crate::value;

use super::util::round_to_precision;

const EARTH_R_IN_M: f64 = 6_371_008.8;
const EARTH_R_IN_KM: f64 = EARTH_R_IN_M / 1000.0;
const EARTH_R_IN_MILES: f64 = EARTH_R_IN_KM * 0.621_371_2;

fn haversine_distance(
    latitude1: Value,
    longitude1: Value,
    latitude2: Value,
    longitude2: Value,
    measurement_unit: &Bytes,
) -> Resolved {
    let lat1 = lat1.try_float()?.to_radians();
    let lon1 = lon1.try_float()?.to_radians();
    let lat2 = lat2.try_float()?.to_radians();
    let lon2 = lon2.try_float()?.to_radians();

    let mut result = ObjectMap::new();

    // Distance calculation
    let dlon = lon2 - lon1;
    let dlat = lat2 - lat1;
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let distance = 2.0 * a.sqrt().asin();

    result.insert(
        "distance".into(),
        match measurement.as_ref() {
            b"kilometers" => {
                Value::from_f64_or_zero(round_to_precision(distance * EARTH_R_IN_KM, 7, f64::round))
            }
            b"miles" => Value::from_f64_or_zero(round_to_precision(
                distance * EARTH_R_IN_MILES,
                7,
                f64::round,
            )),
            _ => unreachable!("enum invariant"),
        },
    );

    // Bearing calculation
    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
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

#[derive(Clone, Copy, Debug)]
pub struct Haversine;

impl Function for Haversine {
    fn identifier(&self) -> &'static str {
        "haversine"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "lat1",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "lon1",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "lat2",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "lon2",
                kind: kind::FLOAT,
                required: true,
            },
            Parameter {
                keyword: "measurement",
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
        let lat1 = arguments.required("lat1");
        let lon1 = arguments.required("lon1");
        let lat2 = arguments.required("lat2");
        let lon2 = arguments.required("lon2");
        let measurement = arguments
            .optional_enum("measurement", &measurement_systems(), state)?
            .unwrap_or_else(|| value!("kilometers"))
            .try_bytes()
            .expect("measurement not bytes");

        Ok(HaversineFn {
            lat1,
            lon1,
            lat2,
            lon2,
            measurement,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "haversine",
                source: "haversine(0, 0, 10, 10)",
                result: Ok(indoc!(
                    r#"{
                        "distance": 1568.5227233,
                        "bearing": 44.561
                    }"#
                )),
            },
            Example {
                title: "haversine in miles",
                source: r#"haversine(0, 0, 10, 10, "miles")"#,
                result: Ok(indoc!(
                    r#"{
                        "distance": 974.6348468
                        "bearing": 44.561
                    }"#
                )),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct HaversineFn {
    lat1: Box<dyn Expression>,
    lon1: Box<dyn Expression>,
    lat2: Box<dyn Expression>,
    lon2: Box<dyn Expression>,
    measurement: Bytes,
}

impl FunctionExpression for HaversineFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let lat1 = self.lat1.resolve(ctx)?;
        let lon1 = self.lon1.resolve(ctx)?;
        let lat2 = self.lat2.resolve(ctx)?;
        let lon2 = self.lon2.resolve(ctx)?;

        haversine(lat1, lon1, lat2, lon2, &self.measurement)
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
            args: func_args![lat1: value!(0.0), lon1: value!(0.0), lat2: value!(10.0), lon2: value!(10.0)],
            want: Ok(value!({ "distance": 1_568.522_723_3, "bearing": 44.561 })),
            tdef: TypeDef::object(inner_kind()).infallible(),
        }

        basic_miles {
            args: func_args![lat1: value!(0.0), lon1: value!(0.0), lat2: value!(10.0), lon2: value!(10.0), measurement: value!("miles")],
            want: Ok(value!({ "distance": 974.634_846_8, "bearing": 44.561 })),
            tdef: TypeDef::object(inner_kind()).infallible(),
        }
    ];
}
