use crate::compiler::prelude::*;

fn zip(value: Value) -> Resolved {
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
        &[Parameter {
            keyword: "array",
            kind: kind::ARRAY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "merge three arrays into an array of 3-tuples",
            source: r#"zip([["a", "b", "c"], [1, null, true], [4, 5, 6]])"#,
            result: Ok(r#"[["a", 1, 4], ["b", null, 5], ["c", true, 6]]"#),
        }]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let array = arguments.required("array");
        Ok(ZipFn { array }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ZipFn {
    array: Box<dyn Expression>,
}

impl FunctionExpression for ZipFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        zip(self.array.resolve(ctx)?)
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
            args: func_args![array: value!([[1, 2, 3], [4, 5, 6]])],
            want: Ok(value!([[1, 4], [2, 5], [3, 6]])),
            tdef: TypeDef::array(Collection::any()),
        }

        zips_three_arrays {
            args: func_args![array: value!([[1, 2, 3], [4, 5, 6], [7, 8, 9]])],
            want: Ok(value!([[1, 4, 7], [2, 5, 8], [3, 6, 9]])),
            tdef: TypeDef::array(Collection::any()),
        }

        uses_shortest_length1 {
            args: func_args![array: value!([[1, 2, 3], [4, 5]])],
            want: Ok(value!([[1, 4], [2, 5]])),
            tdef: TypeDef::array(Collection::any()),
        }

        uses_shortest_length2 {
            args: func_args![array: value!([[1, 2], [4, 5, 6]])],
            want: Ok(value!([[1, 4], [2, 5]])),
            tdef: TypeDef::array(Collection::any()),
        }

        requires_outer_array {
            args: func_args![array: 1],
            want: Err("expected array, got integer"),
            tdef: TypeDef::array(Collection::any()),
        }

        requires_inner_arrays1 {
            args: func_args![array: value!([true, []])],
            want: Err("expected array, got boolean"),
            tdef: TypeDef::array(Collection::any()),
        }

        requires_inner_arrays2 {
            args: func_args![array: value!([[], null])],
            want: Err("expected array, got null"),
            tdef: TypeDef::array(Collection::any()),
        }
    ];
}
