use super::util::ConstOrExpr;
use crate::compiler::prelude::*;

fn zip2(value0: Value, value1: Value) -> Resolved {
    Ok(value0
        .try_array()?
        .into_iter()
        .zip(value1.try_array()?)
        .map(|(v0, v1)| Value::Array(vec![v0, v1]))
        .collect())
}

fn zip_all(value: Value) -> Resolved {
    Ok(MultiZip(
        value
            .try_array()?
            .into_iter()
            .map(|value| value.try_array().map(Vec::into_iter))
            .collect::<Result<_, _>>()?,
    )
    .collect::<Vec<_>>()
    .into())
}

struct MultiZip(Vec<std::vec::IntoIter<Value>>);

impl Iterator for MultiZip {
    type Item = Vec<Value>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.iter_mut().map(Iterator::next).collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Zip;

impl Function for Zip {
    fn identifier(&self) -> &'static str {
        "zip"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "array_0",
                kind: kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "array_1",
                kind: kind::ARRAY,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "merge an array of three arrays into an array of 3-tuples",
                source: r#"zip([["a", "b", "c"], [1, null, true], [4, 5, 6]])"#,
                result: Ok(r#"[["a", 1, 4], ["b", null, 5], ["c", true, 6]]"#),
            },
            Example {
                title: "merge two array parameters",
                source: "zip([1, 2, 3, 4], [5, 6, 7])",
                result: Ok("[[1, 5], [2, 6], [3, 7]]"),
            },
        ]
    }

    fn compile(
        &self,
        state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let array_0 = ConstOrExpr::new(arguments.required("array_0"), state);
        let array_1 = ConstOrExpr::optional(arguments.optional("array_1"), state);

        Ok(ZipFn { array_0, array_1 }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct ZipFn {
    array_0: ConstOrExpr,
    array_1: Option<ConstOrExpr>,
}

impl FunctionExpression for ZipFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let array_0 = self.array_0.resolve(ctx)?;
        match &self.array_1 {
            None => zip_all(array_0),
            Some(array_1) => zip2(array_0, array_1.resolve(ctx)?),
        }
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::array(Collection::any())
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

    use super::*;

    test_function![
        zip => Zip;

        zips_two_arrays {
            args: func_args![array_0: value!([[1, 2, 3], [4, 5, 6]])],
            want: Ok(value!([[1, 4], [2, 5], [3, 6]])),
            tdef: TypeDef::array(Collection::any()),
        }

        zips_three_arrays {
            args: func_args![array_0: value!([[1, 2, 3], [4, 5, 6], [7, 8, 9]])],
            want: Ok(value!([[1, 4, 7], [2, 5, 8], [3, 6, 9]])),
            tdef: TypeDef::array(Collection::any()),
        }

        zips_two_parameters {
            args: func_args![array_0: value!([1, 2, 3]), array_1: value!([4, 5, 6])],
            want: Ok(value!([[1, 4], [2, 5], [3, 6]])),
            tdef: TypeDef::array(Collection::any()),
        }

        uses_shortest_length1 {
            args: func_args![array_0: value!([[1, 2, 3], [4, 5]])],
            want: Ok(value!([[1, 4], [2, 5]])),
            tdef: TypeDef::array(Collection::any()),
        }

        uses_shortest_length2 {
            args: func_args![array_0: value!([[1, 2], [4, 5, 6]])],
            want: Ok(value!([[1, 4], [2, 5]])),
            tdef: TypeDef::array(Collection::any()),
        }

        requires_outer_array {
            args: func_args![array_0: 1],
            want: Err("expected array, got integer"),
            tdef: TypeDef::array(Collection::any()),
        }

        requires_inner_arrays1 {
            args: func_args![array_0: value!([true, []])],
            want: Err("expected array, got boolean"),
            tdef: TypeDef::array(Collection::any()),
        }

        requires_inner_arrays2 {
            args: func_args![array_0: value!([[], null])],
            want: Err("expected array, got null"),
            tdef: TypeDef::array(Collection::any()),
        }
    ];
}
