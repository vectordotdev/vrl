#[macro_export]
macro_rules! test_type_def {
    ($($name:ident { expr: $expr:expr_2021, want: $def:expr_2021, })+) => {
        mod type_def {
            use super::*;

            $(
                #[test]
                fn $name() {
                    let mut state = $crate::compiler::state::TypeState::default();
                    #[allow(clippy::redundant_closure_call)]
                    let expression = Box::new($expr(&mut state));

                    assert_eq!(expression.type_def(&state), $def);
                }
            )+
        }
    };
}

#[macro_export]
macro_rules! func_args {
    () => (
        ::std::collections::HashMap::<&'static str, $crate::value::Value>::default()
    );
    ($($k:tt: $v:expr_2021),+ $(,)?) => {
        vec![$((stringify!($k), $v.into())),+]
            .into_iter()
            .collect::<::std::collections::HashMap<&'static str, $crate::value::Value>>()
    };
}

#[macro_export]
macro_rules! bench_function {
    ($name:tt => $func:path; $($case:ident { args: $args:expr_2021, want: $(Ok($ok:expr_2021))? $(Err($err:expr_2021))? $(,)* })+) => {
        fn $name(c: &mut criterion::Criterion) {
            let mut group = c.benchmark_group(&format!("vrl_stdlib/functions/{}", stringify!($name)));
            group.throughput(criterion::Throughput::Elements(1));
            $(
                group.bench_function(&stringify!($case).to_string(), |b| {
                    let mut state = $crate::compiler::state::TypeState::default();

                    let (expression, want) = $crate::__prep_bench_or_test!($func, &state, $args, $(Ok($crate::value::Value::from($ok)))? $(Err($err.to_owned()))?);
                    let expression = expression.unwrap();
                    let mut runtime_state = $crate::compiler::state::RuntimeState::default();
                    let mut target: $crate::value::Value = ::std::collections::BTreeMap::default().into();
                    let tz = $crate::compiler::TimeZone::Named(chrono_tz::Tz::UTC);
                    let mut ctx = $crate::compiler::Context::new(&mut target, &mut runtime_state, &tz);

                    b.iter(|| {
                        let got = expression.resolve(&mut ctx).map_err(|e| e.to_string());
                        debug_assert_eq!(got, want);
                        got
                    })
                });
            )+
        }
    };
}

#[macro_export]
macro_rules! test_function {

    ($name:tt => $func:path; $($case:ident { args: $args:expr_2021, want: $(Ok($ok:expr_2021))? $(Err($err:expr_2021))?, tdef: $tdef:expr_2021,  $(,)* })+) => {
        test_function!($name => $func; before_each => {} $($case { args: $args, want: $(Ok($ok))? $(Err($err))?, tdef: $tdef, tz: $crate::compiler::TimeZone::Named(chrono_tz::Tz::UTC), })+);
    };

    ($name:tt => $func:path; $($case:ident { args: $args:expr_2021, want: $(Ok($ok:expr_2021))? $(Err($err:expr_2021))?, tdef: $tdef:expr_2021, tz: $tz:expr_2021,  $(,)* })+) => {
        test_function!($name => $func; before_each => {} $($case { args: $args, want: $(Ok($ok))? $(Err($err))?, tdef: $tdef, tz: $tz, })+);
    };

    ($name:tt => $func:path; before_each => $before:block $($case:ident { args: $args:expr_2021, want: $(Ok($ok:expr_2021))? $(Err($err:expr_2021))?, tdef: $tdef:expr_2021,  $(,)* })+) => {
        test_function!($name => $func; before_each => $before $($case { args: $args, want: $(Ok($ok))? $(Err($err))?, tdef: $tdef, tz: $crate::compiler::TimeZone::Named(chrono_tz::Tz::UTC), })+);
    };

    ($name:tt => $func:path; before_each => $before:block $($case:ident { args: $args:expr_2021, want: $(Ok($ok:expr_2021))? $(Err($err:expr_2021))?, tdef: $tdef:expr_2021, tz: $tz:expr_2021,  $(,)* })+) => {
        paste::paste!{$(
            #[test]
            fn [<$name _ $case:snake:lower>]() {
                $before
                let state = $crate::compiler::state::TypeState::default();

                let (expression, want) = $crate::__prep_bench_or_test!($func, &state, $args, $(Ok($crate::value::Value::from($ok)))? $(Err($err.to_owned()))?);
                match expression {
                    Ok(expression) => {
                        let mut runtime_state = $crate::compiler::state::RuntimeState::default();
                        let mut target: $crate::value::Value = ::std::collections::BTreeMap::default().into();
                        let tz = $tz;
                        let mut ctx = $crate::compiler::Context::new(&mut target, &mut runtime_state, &tz);

                        let got_value = expression.resolve(&mut ctx)
                            .map_err(|e| format!("{:#}", anyhow::anyhow!(e)));

                        assert!(got_value == want, "assertion failed for `{}` case:\n  got:    {:?}\n  wanted: {:?}", stringify!($case), got_value, want);
                        let got_tdef = expression.type_def(&state);
                        assert_eq!(got_tdef, $tdef);
                    }
                    err@Err(_) => {
                        // Allow tests against compiler errors.
                        assert_eq!(err
                                   // We have to map to a value just to make sure the types match even though
                                   // it will never be used.
                                   .map(|_| $crate::value::Value::Null)
                                   .map_err(|e| format!("{:#}", e.message())), want);
                    }
                }
            }
        )+}
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __prep_bench_or_test {
    ($func:path, $state:expr_2021, $args:expr_2021, $want:expr_2021) => {{
        let config = $crate::compiler::CompileConfig::default();
        (
            $func.compile(
                $state,
                &mut $crate::compiler::function::FunctionCompileContext::new(
                    $crate::diagnostic::Span::new(0, 0),
                    config,
                ),
                $args.into(),
            ),
            $want,
        )
    }};
}

#[macro_export]
macro_rules! type_def {
    (unknown) => {
        TypeDef::any()
    };

    (bytes) => {
        TypeDef::bytes()
    };

    (object {$(unknown => $unknown:expr_2021,)? $($key:literal => $value:expr_2021,)+ }) => {{
        #[allow(unused_mut)]
        let mut v = $crate::value::kind::Collection::from(::std::collections::BTreeMap::from([$(($key.into(), $value.into()),)+]));
        $(v.set_unknown($crate::value::Kind::from($unknown));)?

        TypeDef::object(v)
    }};

    (array [ $($value:expr_2021,)+ ]) => {{
        $(let v = $crate::value::kind::Collection::from_unknown($crate::value::Kind::from($value));)+

        TypeDef::array(v)
    }};

    (array { $(unknown => $unknown:expr_2021,)? $($idx:literal => $value:expr_2021,)+ }) => {{
        let mut v = $crate::value::kind::Collection::from(::std::collections::BTreeMap::from([$(($idx.into(), $value.into()),)+]));
        $(v.set_unknown($crate::value::Kind::from($unknown));)?

        TypeDef::array(v)
    }};

    (array) => {
        TypeDef::array($crate::value::kind::Collection::any())
    };
}
