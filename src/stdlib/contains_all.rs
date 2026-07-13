use crate::compiler::prelude::*;
use crate::stdlib::string_utils::convert_to_string;

fn contains_all(value: &Value, substrings: Value, case_sensitive: Option<Value>) -> Resolved {
    let case_sensitive = match case_sensitive {
        Some(v) => v.try_boolean()?,
        None => true,
    };

    let value_string = convert_to_string(value, !case_sensitive)?;
    let substring_values = substrings.try_array()?;

    for substring_value in substring_values {
        let substring = convert_to_string(&substring_value, !case_sensitive)?;
        if !value_string.contains(substring.as_ref()) {
            return Ok(false.into());
        }
    }
    Ok(true.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ContainsAll;

impl Function for ContainsAll {
    fn identifier(&self) -> &'static str {
        "contains_all"
    }

    fn usage(&self) -> &'static str {
        "Determines whether the `value` string contains all the specified `substrings`."
    }

    fn category(&self) -> &'static str {
        Category::String.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required("value", kind::BYTES, "The text to search."),
            Parameter::required(
                "substrings",
                kind::ARRAY,
                "An array of substrings to search for in `value`.",
            )
            .with_element_kind(kind::BYTES),
            Parameter::optional(
                "case_sensitive",
                kind::BOOLEAN,
                "Whether the match should be case sensitive.",
            ),
        ];
        PARAMETERS
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let substrings = arguments.required("substrings");
        let case_sensitive = arguments.optional("case_sensitive");

        Ok(ContainsAllFn {
            value,
            substrings,
            case_sensitive,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "String contains all with default parameters (case sensitive)",
                source: r#"contains_all("The NEEDLE in the Haystack", ["NEEDLE", "Haystack"])"#,
                result: Ok("true"),
            },
            example! {
                title: "String doesn't contain all with default parameters (case sensitive)",
                source: r#"contains_all("The NEEDLE in the Haystack", ["needle", "Haystack"])"#,
                result: Ok("false"),
            },
            example! {
                title: "String contains all (case insensitive)",
                source: r#"contains_all("The NEEDLE in the HaYsTaCk", ["nEeDlE", "haystack"], case_sensitive: false)"#,
                result: Ok("true"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct ContainsAllFn {
    value: Box<dyn Expression>,
    substrings: Box<dyn Expression>,
    case_sensitive: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ContainsAllFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let substrings = self.substrings.resolve(ctx)?;
        let case_sensitive = self
            .case_sensitive
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;
        contains_all(&value, substrings, case_sensitive)
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        // Element-kind constraint on `substrings` is declared via
        // `Parameter::with_element_kind`, which drives compiler-level
        // fallibility inference at the call site.
        TypeDef::boolean()
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

    use super::*;

    test_function![
        contains_all => ContainsAll;

        no {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["the", "duck"])],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        // The function body's `type_def` is now unconditionally infallible;
        // element-kind fallibility for `substrings` is inferred by the compiler
        // at the call site via `Parameter::with_element_kind`. See the
        // `compiler_flags_non_bytes_element_as_fallible` test for end-to-end
        // verification.
        substring_type {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!([1, 2])],
            want: Err("expected string, got integer"),
            tdef: TypeDef::boolean(),
        }

        yes {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["The Needle", "Needle In"])],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_sensitive_yes {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["Needle", "Haystack"])],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

         case_sensitive_no {
                        args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["needle", "haystack"])],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_insensitive_no {
                        args: func_args![value: value!("The Needle In The Haystack"),
                                        substrings: value!(["thread", "haystack"]),
                                        case_sensitive: false],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_insensitive_yes {
                       args: func_args![value: value!("The Needle In The Haystack"),
                                        substrings: value!(["needle", "haystack"]),
                                        case_sensitive: false],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];

    /// End-to-end check: `Parameter::with_element_kind(kind::BYTES)` should drive
    /// the compiler to mark a `contains_all` call fallible when it cannot prove
    /// the array elements are strings.
    #[test]
    fn compiler_flags_non_bytes_element_as_fallible() {
        let fns = vec![Box::new(ContainsAll) as Box<dyn crate::compiler::Function>];

        // All string literals: element kind is a subset of `bytes` -> infallible.
        let src = r#"contains_all("The Needle", ["Needle", "Hay"])"#;
        let res = crate::compiler::compile(src, &fns).expect("compiles");
        assert!(
            !res.program.info().fallible,
            "call with string literals should be infallible"
        );

        // Element kind cannot be proven a subset of `bytes` -> fallible.
        // [1, 2] has element kind integer, which is provably disjoint from
        // bytes. The compiler now rejects this outright — even `??` cannot
        // silence a provably-impossible argument.
        let src = r#"contains_all("The Needle", [1, 2]) ?? false"#;
        assert!(
            crate::compiler::compile(src, &fns).is_err(),
            "[1, 2] is provably not array<bytes>; must be a compile error even with `??`"
        );

        let src = r#"contains_all("The Needle", [1, 2])"#;
        assert!(
            crate::compiler::compile(src, &fns).is_err(),
            "unhandled provably-wrong call should fail to compile"
        );
    }
}
