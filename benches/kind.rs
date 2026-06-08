use std::fmt;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::compiler::value::kind;
use vrl::value::Value;

struct Parameters {
    basis: u16,
}

static PARAMETERS: [Parameters; 4] = [
    Parameters { basis: kind::BYTES },
    Parameters { basis: kind::ARRAY },
    Parameters { basis: kind::REGEX },
    Parameters { basis: kind::NULL },
];

impl fmt::Display for Parameters {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.basis)
    }
}

fn benchmark_kind_display(c: &mut Criterion) {
    let mut group = c.benchmark_group("vrl_compiler/value::kind::display");
    for param in &PARAMETERS {
        let parameter = vrl::compiler::Parameter::optional("", param.basis, "");

        let kind = parameter.kind();

        group.bench_with_input(BenchmarkId::from_parameter(param), &kind, |b, kind| {
            b.iter(|| kind.to_string())
        });
    }
}

// Benchmarks the full realistic error path: constructing a Kind from a live Value
// and formatting it into an error message, as happens in e.g. `bool([1]) ?? false`.
fn benchmark_kind_from_value_display(c: &mut Criterion) {
    let mut group = c.benchmark_group("vrl_compiler/value::kind::from_value_display");

    let array_value: Value = (0_i64..10).map(Value::Integer).collect::<Vec<_>>().into();
    group.bench_function("array_10_elems", |b| {
        b.iter(|| {
            format!("expected boolean, got {}", array_value.kind())
        })
    });

    let object_value: Value = (0_i64..10)
        .map(|i| (vrl::value::KeyString::from(format!("k{i}")), Value::Integer(i)))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into();
    group.bench_function("object_10_keys", |b| {
        b.iter(|| {
            format!("expected boolean, got {}", object_value.kind())
        })
    });

    let bytes_value: Value = Value::from("hello");
    group.bench_function("bytes", |b| {
        b.iter(|| {
            format!("expected boolean, got {}", bytes_value.kind())
        })
    });
}

criterion_group!(name = vrl_compiler_kind;
                 config = Criterion::default();
                 targets = benchmark_kind_display, benchmark_kind_from_value_display);
criterion_main!(vrl_compiler_kind);
