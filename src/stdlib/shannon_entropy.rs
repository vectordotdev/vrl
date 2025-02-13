use std::{collections::HashMap, str::FromStr};

use unicode_segmentation::UnicodeSegmentation;

use crate::compiler::prelude::*;

// Casting to f64 in this function is only done to enable proper division (when calculating probability)
// Since numbers being casted represent lenghts of input strings and number of character occurences,
// we can assume that there will never really be precision loss here, because that would mean that
// the string is at least 2^52 bytes in size (4.5 PB)
#[allow(clippy::cast_precision_loss)]
fn shannon_entropy(value: &Value, segmentation: &Segmentation) -> Resolved {
    let (occurence_counts, total_length): (Vec<usize>, usize) = match segmentation {
        Segmentation::Byte => {
            // Optimized version for bytes, since there is a limited number of options, that could
            // easily be kept track of
            let bytes = value.clone().try_bytes()?;
            let mut counts = [0usize; 256];
            let total_len = bytes.len() as f64;

            for b in bytes {
                counts[b as usize] += 1;
            }

            let mut entropy = 0.0;

            for count in counts {
                if count == 0 {
                    continue;
                }

                let p = count as f64 / total_len;

                entropy -= p * p.log2();
            }

            return Ok(Value::from_f64_or_zero(entropy));
        }
        Segmentation::Codepoint => {
            let string = value.try_bytes_utf8_lossy()?;
            let chars = string.chars();
            let mut counts = HashMap::new();
            let mut total_len = 0;

            for char in chars {
                counts.entry(char).and_modify(|c| *c += 1).or_insert(1);
                total_len += 1;
            }

            (counts.into_values().collect(), total_len)
        }
        Segmentation::Grapheme => {
            let string = value.try_bytes_utf8_lossy()?;
            let graphemes = string.graphemes(true);
            let mut counts = HashMap::new();
            let mut total_len = 0;

            for grapheme in graphemes {
                counts.entry(grapheme).and_modify(|c| *c += 1).or_insert(1);
                total_len += 1;
            }

            (counts.into_values().collect(), total_len)
        }
    };

    Ok(Value::from_f64_or_zero(
        occurence_counts
            .iter()
            // Calculate probability of each item by diving occurence count by total length
            .map(|occurence_count| *occurence_count as f64 / total_length as f64)
            // Calculate entropy using definition: sum(-p * log2(p))
            .fold(0f64, |acc, p| acc - (p * p.log2())),
    ))
}

#[derive(Default, Debug, Clone)]
enum Segmentation {
    #[default]
    Byte,
    Codepoint,
    Grapheme,
}

impl FromStr for Segmentation {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "byte" => Ok(Self::Byte),
            "codepoint" => Ok(Self::Codepoint),
            "grapheme" => Ok(Self::Grapheme),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShannonEntropy;

impl Function for ShannonEntropy {
    fn identifier(&self) -> &'static str {
        "shannon_entropy"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "segmentation",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "shannon_entropy simple",
                source: r#"floor(shannon_entropy("vector.dev"), precision: 4)"#,
                result: Ok("2.9219"),
            },
            Example {
                title: "shannon_entropy UTF-8 wrong segmentation",
                source: r#"floor(shannon_entropy("test123%456.فوائد.net."), precision: 4)"#,
                result: Ok("4.0784"),
            },
            Example {
                title: "shannon_entropy UTF-8 grapheme segmentation",
                source: r#"shannon_entropy("👨‍👩‍👧‍👦", segmentation: "grapheme")"#,
                result: Ok("0.0"),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let segmentation = arguments
            .optional_enum(
                "segmentation",
                &["byte".into(), "codepoint".into(), "grapheme".into()],
                state,
            )?
            .map(|s| {
                Segmentation::from_str(&s.try_bytes_utf8_lossy().expect("segmentation not bytes"))
                    .expect("validated enum")
            })
            .unwrap_or_default();

        Ok(ShannonEntropyFn {
            value,
            segmentation,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct ShannonEntropyFn {
    value: Box<dyn Expression>,
    segmentation: Segmentation,
}

impl FunctionExpression for ShannonEntropyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        shannon_entropy(&value, &self.segmentation)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::float().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        shannon_entropy => ShannonEntropy;

        simple_example {
            args: func_args![value: value!("vector.dev")],
            want: Ok(value!(2.921_928_094_887_362_3)),
            tdef: TypeDef::float().infallible(),
        }

        longer_example {
            args: func_args![value: value!("Supercalifragilisticexpialidocious")],
            want: Ok(value!(3.736_987_930_635_820_5)),
            tdef: TypeDef::float().infallible(),
        }

        fancy_foo_example {
            args: func_args![value: value!("ƒoo")],
            want: Ok(value!(1.5)),
            tdef: TypeDef::float().infallible(),
        }

        fancy_foo_codepoint_segmentation_example {
            args: func_args![value: value!("ƒoo"), segmentation: value!("codepoint")],
            want: Ok(value!(0.918_295_834_054_489_6)),
            tdef: TypeDef::float().infallible(),
        }

        utf_8_byte_segmentation_example {
            args: func_args![value: value!("test123%456.فوائد.net.")],
            want: Ok(value!(4.078_418_520_441_603)),
            tdef: TypeDef::float().infallible(),
        }

        utf_8_codepoint_segmentation_example {
            args: func_args![value: value!("test123%456.فوائد.net."), segmentation: value!("codepoint")],
            want: Ok(value!(3.936_260_027_531_526_3)),
            tdef: TypeDef::float().infallible(),
        }

        utf_8_example {
            args: func_args![value: value!("test123%456.فوائد.net."), segmentation: value!("grapheme")],
            want: Ok(value!(3.936_260_027_531_526_3)),
            tdef: TypeDef::float().infallible(),
        }
    ];
}
