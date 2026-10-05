use std::{fmt, sync::Arc};

use super::Block;
use crate::compiler::codes;
use crate::compiler::expression::function_call::Warning::AbortInfallible;
use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    CompileConfig, Context, Expression, Function, Resolved, Span, TypeDef,
    expression::{ExpressionError, FunctionArgument, levenstein},
    function::{
        ArgumentList, Closure, Example, FunctionCompileContext, Parameter,
        closure::{self, VariableKind},
    },
    parser::{Ident, Node},
    state::LocalEnv,
    type_def::Details,
    value::Kind,
};
use crate::diagnostic::{DiagnosticMessage, Label, Note, Severity, Urls};
use crate::prelude::Note::SeeErrorDocs;
use crate::value::Value;

#[derive(Clone, Copy)]
struct ArgumentParameter {
    index: usize,
    parameter: Parameter,
}

struct AppliedArgument {
    type_def: TypeDef,
    preceded_by_side_effects: bool,
}

pub(crate) struct Builder<'a> {
    abort_on_error: bool,
    argument_parameters: Vec<ArgumentParameter>,
    arguments_with_unknown_type_validity: Vec<(Parameter, Node<FunctionArgument>, Kind)>,
    call_span: Span,
    ident_span: Span,
    function_id: usize,
    arguments: Arc<Vec<Node<FunctionArgument>>>,
    closure: Option<(Vec<Ident>, closure::Input)>,
    list: ArgumentList,
    function: &'a dyn Function,
}

pub(crate) struct CallCompilationResult {
    pub(crate) function_call: FunctionCall,
    pub(crate) error: Option<FunctionCallError>,
}

impl<'a> Builder<'a> {
    pub(crate) fn get_arg_list(&self) -> &ArgumentList {
        &self.list
    }

    pub(crate) fn supports_break(&self) -> bool {
        self.function
            .closure()
            .is_some_and(|def| def.supports_break)
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        call_span: Span,
        ident: Node<Ident>,
        abort_on_error: bool,
        arguments: Vec<Node<FunctionArgument>>,
        funcs: &'a [Box<dyn Function>],
        state_before_function_args: &TypeState,
        state: &mut TypeState,
        closure_variables: Option<Node<Vec<Node<Ident>>>>,
    ) -> Result<Self, FunctionCallError> {
        let (ident_span, ident) = ident.take();

        // Check if function exists.
        let Some((function_id, function)) = funcs
            .iter()
            .enumerate()
            .find(|(_pos, f)| f.identifier() == ident.as_ref())
        else {
            let idents = funcs
                .iter()
                .map(|func| func.identifier())
                .collect::<Vec<_>>();

            return Err(FunctionCallError::Undefined {
                ident_span,
                ident: ident.clone(),
                idents,
            });
        };

        // Check function arity.
        if arguments.len() > function.parameters().len() {
            let arguments_span = {
                let start = arguments.first().unwrap().span().start();
                let end = arguments.last().unwrap().span().end();

                Span::new(start, end)
            };

            return Err(FunctionCallError::WrongNumberOfArgs {
                arguments_span,
                max: function.parameters().len(),
            });
        }

        // Keeps track of positional argument indices.
        //
        // Used to map a positional argument to its keyword. Keyword arguments
        // can be used in any order, and don't count towards the index of
        // positional arguments.
        let mut index = 0;
        let mut list = ArgumentList::default();

        let mut argument_parameters = Vec::with_capacity(arguments.len());
        for node in &arguments {
            let argument = node.inner();
            let (parameter_index, parameter) = match argument.keyword() {
                // positional argument
                None => {
                    let parameter_index = index;
                    index += 1;
                    function
                        .parameters()
                        .get(parameter_index)
                        .map(|parameter| (parameter_index, parameter))
                }

                // keyword argument
                Some(k) => function
                    .parameters()
                    .iter()
                    .enumerate()
                    .find(|(_, param)| param.keyword == k)
                    .map(|(parameter_index, parameter)| {
                        if parameter_index == index {
                            index += 1;
                        }

                        (parameter_index, parameter)
                    }),
            }
            .ok_or_else(|| FunctionCallError::UnknownKeyword {
                keyword_span: argument.keyword_span().expect("exists"),
                ident_span,
                keywords: function.parameters().iter().map(|p| p.keyword).collect(),
            })?;

            argument_parameters.push(ArgumentParameter {
                index: parameter_index,
                parameter: *parameter,
            });
        }

        let mut state_before_argument = state_before_function_args.clone();
        let applied_arguments = apply_arguments_for_type_check(
            &arguments,
            &argument_parameters,
            &mut state_before_argument,
        );

        let mut arguments_with_unknown_type_validity = vec![];
        for ((node, argument_parameter), applied_argument) in arguments
            .iter()
            .zip(&argument_parameters)
            .zip(applied_arguments)
        {
            let (argument_span, argument) = node.clone().take();
            let parameter = &argument_parameter.parameter;

            // Check if the argument is of the expected type.
            let expr_kind = applied_argument.type_def.kind();
            let base_param_kind = parameter.kind_without_element_constraint();
            let param_kind = parameter.kind();
            let runtime_kind_is_valid = param_kind.is_superset(expr_kind).is_ok();
            // If an earlier parameter can make a constrained argument invalid, retain runtime
            // fallibility. Re-check the initial kind only for that candidate, avoiding a second
            // recursive pass over every argument.
            let defer_element_kind_check = parameter.has_element_kind_constraint()
                && !runtime_kind_is_valid
                && applied_argument.preceded_by_side_effects
                && param_kind
                    .intersects(argument.expr().type_def(state_before_function_args).kind());

            if !base_param_kind.intersects(expr_kind)
                || (!param_kind.intersects(expr_kind) && !defer_element_kind_check)
            {
                return Err(FunctionCallError::InvalidArgumentKind(
                    InvalidArgumentErrorContext {
                        function_ident: function.identifier(),
                        abort_on_error,
                        arguments_fmt: arguments
                            .iter()
                            .map(|arg| arg.inner().to_string())
                            .collect::<Vec<_>>(),
                        parameter: *parameter,
                        got: expr_kind.clone(),
                        argument,
                        argument_span,
                    },
                ));
            } else if !runtime_kind_is_valid {
                arguments_with_unknown_type_validity.push((
                    *parameter,
                    node.clone(),
                    expr_kind.clone(),
                ));
            }

            // Check if the argument is infallible.
            if applied_argument.type_def.is_fallible() {
                return Err(FunctionCallError::FallibleArgument {
                    expr_span: argument.span(),
                });
            }

            list.insert(parameter.keyword, argument.into_inner());
        }

        // Check missing required arguments.
        function
            .parameters()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.required)
            .filter(|(_, p)| !list.keywords().contains(&p.keyword))
            .try_for_each(|(i, p)| -> Result<_, _> {
                Err(FunctionCallError::MissingArgument {
                    call_span,
                    keyword: p.keyword,
                    position: i,
                })
            })?;

        // Check function closure validity.
        let closure = Self::check_closure(
            function.as_ref(),
            closure_variables,
            call_span,
            &list,
            state,
            ident_span,
        )?;

        Ok(Self {
            abort_on_error,
            argument_parameters,
            arguments_with_unknown_type_validity,
            call_span,
            ident_span,
            function_id,
            arguments: Arc::new(arguments),
            closure,
            list,
            function: function.as_ref(),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn check_closure(
        function: &dyn Function,
        closure_variables: Option<Node<Vec<Node<Ident>>>>,
        call_span: Span,
        list: &ArgumentList,
        state: &mut TypeState,
        ident_span: Span,
    ) -> Result<Option<(Vec<Ident>, closure::Input)>, FunctionCallError> {
        let closure = match (function.closure(), closure_variables) {
            // Error if closure is provided for function that doesn't support
            // any.
            (None, Some(variables)) => {
                let closure_span = variables.span();

                return Err(FunctionCallError::UnexpectedClosure {
                    call_span,
                    closure_span,
                });
            }

            // Error if closure is missing from function that expects one.
            (Some(definition), None) => {
                let example = definition.inputs.first().map(|input| input.example);

                return Err(FunctionCallError::MissingClosure { call_span, example });
            }

            // Check for invalid closure signature.
            (Some(definition), Some(variables)) => {
                let mut matched = None;
                let mut err_found_type_def = None;

                for input in definition.inputs {
                    // Check type definition for linked parameter.
                    match list.arguments.get(input.parameter_keyword) {
                        // No argument provided for the given parameter keyword.
                        //
                        // This means the closure can't act on the input
                        // definition, so we continue on to the next. If no
                        // input definitions are valid, the closure is invalid.
                        None => (),

                        // We've found the function argument over which the
                        // closure is going to resolve. We need to ensure the
                        // type of this argument is as expected by the closure.
                        Some(expr) => {
                            let type_def = expr.type_def(state);
                            // The type definition of the value does not match
                            // the expected closure type, continue to check if
                            // the closure eventually accepts this definition.
                            //
                            // Keep track of the type information, so that we
                            // can report these in a diagnostic error if no
                            // other input definition matches.
                            if input.kind.is_superset(type_def.kind()).is_err() {
                                err_found_type_def = Some(type_def.kind().clone());
                                continue;
                            }

                            matched = Some((input, expr));
                            break;
                        }
                    }
                }

                // None of the inputs matched the value type, this is a user error.
                match matched {
                    None => {
                        return Err(FunctionCallError::ClosureParameterTypeMismatch {
                            call_span,
                            found_kind: err_found_type_def.unwrap_or_else(Kind::any),
                        });
                    }

                    Some((input, target)) => {
                        // Now that we know we have a matching parameter argument with a valid type
                        // definition, we can move on to checking/defining the closure arguments.
                        //
                        // In doing so we:
                        //
                        // - check the arity of the closure arguments
                        // - set the expected type definition of each argument
                        if input.variables.len() != variables.len() {
                            let closure_arguments_span =
                                variables.first().map_or(call_span, |node| {
                                    (node.span().start(), variables.last().unwrap().span().end())
                                        .into()
                                });

                            return Err(FunctionCallError::ClosureArityMismatch {
                                ident_span,
                                closure_arguments_span,
                                expected: input.variables.len(),
                                supplied: variables.len(),
                            });
                        }

                        // Get the provided argument identifier in the same position as defined in the
                        // input definition.
                        //
                        // That is, if the function closure definition expects:
                        //
                        //   [bytes, integer]
                        //
                        // Then, given for an actual implementation of:
                        //
                        //   foo() -> { |bar, baz| }
                        //
                        // We set "bar" (index 0) to return bytes, and "baz" (index 1) to return an
                        // integer.
                        for (index, input_var) in input.variables.clone().into_iter().enumerate() {
                            let call_ident = &variables[index];
                            let type_def = target.type_info(state).result;

                            let (type_def, value) = match input_var.kind {
                                // The variable kind is expected to be exactly
                                // the kind provided by the closure definition.
                                VariableKind::Exact(kind) => (kind.into(), None),

                                // The variable kind is expected to be equal to
                                // the ind of the target of the closure.
                                VariableKind::Target => (
                                    target.type_info(state).result,
                                    target.resolve_constant(state),
                                ),

                                // The variable kind is expected to be equal to
                                // the reduced kind of all values within the
                                // target collection type.
                                //
                                // This assumes the target is a collection type,
                                // or else it'll return "any".
                                VariableKind::TargetInnerValue => {
                                    let kind = if let Some(object) = type_def.as_object() {
                                        object.reduced_kind()
                                    } else if let Some(array) = type_def.as_array() {
                                        array.reduced_kind()
                                    } else {
                                        Kind::any()
                                    };

                                    (kind.into(), None)
                                }

                                // The variable kind is expected to be equal to
                                // the kind of all keys within the target
                                // collection type.
                                //
                                // This means it's either a string for an
                                // object, integer for an array, or
                                // a combination of the two if the target isn't
                                // known to be exactly one of the two.
                                //
                                // If the target can resolve to a non-collection
                                // type, this again returns "any".
                                VariableKind::TargetInnerKey => {
                                    let mut kind = Kind::never();

                                    if type_def.is_collection() {
                                        if type_def.is_object() {
                                            kind.add_bytes();
                                        }
                                        if type_def.is_array() {
                                            kind.add_integer();
                                        }
                                    } else {
                                        kind = Kind::any();
                                    }

                                    (kind.into(), None)
                                }
                            };

                            let details = Details { type_def, value };

                            state
                                .local
                                .insert_variable(call_ident.clone().into_inner(), details);
                        }

                        let variables = variables
                            .into_inner()
                            .into_iter()
                            .map(Node::into_inner)
                            .collect();

                        Some((variables, input))
                    }
                }
            }

            _ => None,
        };
        Ok(closure)
    }

    pub(crate) fn compile(
        mut self,
        state_before_function_args: &TypeState,
        state: &mut TypeState,
        closure_block: Option<Node<(Block, TypeDef)>>,
        local_snapshot: LocalEnv,
        config: &mut CompileConfig,
    ) -> Result<CallCompilationResult, FunctionCallError> {
        let (closure, closure_fallible) =
            self.compile_closure(closure_block, local_snapshot, state)?;

        let call_span = self.call_span;
        let ident_span = self.ident_span;

        // We take the external context, and pass it to the function compile context, this allows
        // functions mutable access to external state, but keeps the internal compiler state behind
        // an immutable reference, to ensure compiler state correctness.
        let temp_config = std::mem::take(config);

        let mut compile_ctx = FunctionCompileContext::new(self.call_span, temp_config);

        let expr = self
            .function
            .compile(
                state_before_function_args,
                &mut compile_ctx,
                self.list.clone(),
            )
            .map_err(|error| FunctionCallError::Compilation { call_span, error })?;

        // Re-insert the external context into the compiler state.
        *config = compile_ctx.into_config();

        // Asking for an infallible function to abort on error makes no sense.
        // We consider this an error at compile-time, because it makes the
        // resulting program incorrectly convey this function call might fail.
        let mut state_after_arguments = state_before_function_args.clone();
        let arguments_may_fail_type_check = apply_argument_type_info(
            &self.arguments,
            &self.argument_parameters,
            &mut state_after_arguments,
        );

        let mut warnings = Vec::new();
        if self.abort_on_error
            && !arguments_may_fail_type_check
            && !expr.type_info(&state_after_arguments).result.is_fallible()
        {
            warnings.push(AbortInfallible {
                ident_span,
                abort_span: Span::new(ident_span.end(), ident_span.end() + 1),
            });
        }

        // The function is expected to abort at boot-time if any error occurred,
        // and one or more arguments are of an invalid type, so we'll return the
        // appropriate error.
        let mut invalid_argument_error = None;
        if let Some((parameter, argument, got)) =
            self.arguments_with_unknown_type_validity.first().cloned()
            && !self.abort_on_error
        {
            invalid_argument_error = Some(FunctionCallError::InvalidArgumentKind(
                InvalidArgumentErrorContext {
                    function_ident: self.function.identifier(),
                    abort_on_error: self.abort_on_error,
                    arguments_fmt: self
                        .arguments
                        .iter()
                        .map(|arg| arg.inner().to_string())
                        .collect::<Vec<_>>(),
                    parameter,
                    got,
                    argument: argument.clone().into_inner(),
                    argument_span: argument
                        .keyword_span()
                        .unwrap_or_else(|| argument.expr_span()),
                },
            ));
        }

        Ok(CallCompilationResult {
            function_call: FunctionCall {
                abort_on_error: self.abort_on_error,
                expr,
                argument_parameters: self.argument_parameters,
                closure_fallible,
                closure,
                span: call_span,
                ident: self.function.identifier(),
                function_id: self.function_id,
                arguments: self.arguments.clone(),
                warnings,
            },
            error: invalid_argument_error,
        })
    }

    fn compile_closure(
        &mut self,
        closure_block: Option<Node<(Block, TypeDef)>>,
        mut locals: LocalEnv,
        state: &mut TypeState,
    ) -> Result<(Option<Closure>, bool), FunctionCallError> {
        // Check if we have a closure we need to compile.
        if let Some((variables, input)) = self.closure.clone() {
            // TODO: This assumes the closure will run exactly once, which is incorrect.
            // see: https://github.com/vectordotdev/vector/issues/13782

            let block = closure_block.expect("closure must contain block");

            // At this point, we've compiled the block, so we can remove the
            // closure variables from the compiler's local environment.
            for ident in &variables {
                match locals.remove_variable(ident) {
                    Some(details) => state.local.insert_variable(ident.clone(), details),
                    None => {
                        state.local.remove_variable(ident);
                    }
                }
            }

            let (block_span, (block, block_type_def)) = block.take();

            let closure_fallible = block_type_def.is_fallible();

            // Check the type definition of the resulting block.This needs to match
            // whatever is configured by the closure input type.
            let expected_kind = input.output.into_kind();
            let found_kind = block_type_def
                .kind()
                .union(block_type_def.returns().clone());

            if expected_kind.is_superset(&found_kind).is_err() {
                return Err(FunctionCallError::ReturnTypeMismatch {
                    block_span,
                    found_kind,
                    expected_kind,
                });
            }

            let fnclosure = Closure::new(variables, block, block_type_def);
            self.list.set_closure(fnclosure.clone());

            // closure = Some(fnclosure);
            Ok((Some(fnclosure), closure_fallible))
        } else {
            Ok((None, false))
        }
    }
}

fn apply_arguments_for_type_check(
    arguments: &[Node<FunctionArgument>],
    parameters: &[ArgumentParameter],
    state: &mut TypeState,
) -> Vec<AppliedArgument> {
    debug_assert_eq!(arguments.len(), parameters.len());

    let mut indices = (0..arguments.len()).collect::<Vec<_>>();
    // Functions with element-kind constraints resolve arguments in parameter order. Other
    // functions do not yet share a runtime ordering contract, so preserve the compiler's prior
    // source-order behavior for them.
    if parameters
        .iter()
        .any(|parameter| parameter.parameter.has_element_kind_constraint())
    {
        indices.sort_by_key(|&index| parameters[index].index);
    }

    let mut applied_arguments = (0..arguments.len()).map(|_| None).collect::<Vec<_>>();
    let mut preceded_by_side_effects = false;
    for index in indices {
        let state_before_argument = state.clone();
        let type_def = arguments[index].inner().expr().apply_type_info(state);
        let has_side_effects = *state != state_before_argument;
        applied_arguments[index] = Some(AppliedArgument {
            type_def,
            preceded_by_side_effects,
        });
        preceded_by_side_effects |= has_side_effects;
    }

    applied_arguments
        .into_iter()
        .map(|argument| argument.expect("each argument has a parameter"))
        .collect()
}

fn apply_argument_type_info(
    arguments: &[Node<FunctionArgument>],
    parameters: &[ArgumentParameter],
    state: &mut TypeState,
) -> bool {
    apply_arguments_for_type_check(arguments, parameters, state)
        .into_iter()
        .zip(parameters)
        .any(|(argument, parameter)| {
            parameter
                .parameter
                .kind()
                .is_superset(argument.type_def.kind())
                .is_err()
        })
}

#[derive(Clone)]
pub struct FunctionCall {
    abort_on_error: bool,
    expr: Box<dyn Expression>,
    argument_parameters: Vec<ArgumentParameter>,
    closure_fallible: bool,
    // will be used with: https://github.com/vectordotdev/vector/issues/13782
    #[allow(dead_code)]
    closure: Option<Closure>,

    // used for enhancing runtime error messages (using abort-instruction).
    //
    // TODO: have span store line/col details to further improve this.
    pub(crate) span: Span,

    // used for equality check
    pub(crate) ident: &'static str,

    // May be used by the LLVM runtime. If not, it should be removed
    #[allow(dead_code)]
    function_id: usize,
    arguments: Arc<Vec<Node<FunctionArgument>>>,

    pub(crate) warnings: Vec<Warning>,
}

impl FunctionCall {
    /// Takes the arguments passed and resolves them into the order they are defined
    /// in the function
    /// The error path in this function should never really be hit as the compiler should
    /// catch these whilst creating the AST.
    // May be used by the LLVM runtime. If not, it should be removed
    #[allow(dead_code)]
    fn resolve_arguments(
        &self,
        function: &dyn Function,
    ) -> Result<Vec<(&'static str, Option<FunctionArgument>)>, String> {
        let params = function.parameters().to_vec();
        let mut result = params
            .iter()
            .map(|param| (param.keyword, None))
            .collect::<Vec<_>>();

        let mut unnamed = Vec::new();

        // Position all the named parameters, keeping track of all the unnamed for later.
        for param in self.arguments.iter() {
            match param.keyword() {
                None => unnamed.push(param.clone().take().1),
                Some(keyword) => {
                    match params.iter().position(|param| param.keyword == keyword) {
                        None => {
                            // The parameter was not found in the list.
                            return Err(format!("parameter {keyword} not found."));
                        }
                        Some(pos) => {
                            result[pos].1 = Some(param.clone().take().1);
                        }
                    }
                }
            }
        }

        // Position all the remaining unnamed parameters
        let mut pos = 0;
        for param in unnamed {
            while result[pos].1.is_some() {
                pos += 1;
            }

            if pos > result.len() {
                return Err("Too many parameters".to_string());
            }

            result[pos].1 = Some(param);
        }

        Ok(result)
    }

    #[must_use]
    pub fn arguments_fmt(&self) -> Vec<String> {
        self.arguments
            .iter()
            .map(|arg| arg.inner().to_string())
            .collect::<Vec<_>>()
    }

    #[must_use]
    pub fn arguments_dbg(&self) -> Vec<String> {
        self.arguments
            .iter()
            .map(|arg| format!("{:?}", arg.inner()))
            .collect::<Vec<_>>()
    }
}

impl Expression for FunctionCall {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.expr.resolve(ctx).map_err(|err| match err {
            ExpressionError::Interrupted
            | ExpressionError::Abort { .. }
            | ExpressionError::Break { .. }
            | ExpressionError::Fallible { .. }
            | ExpressionError::Missing { .. } => {
                // propagate the error
                err
            }
            ExpressionError::Return { span, .. } => ExpressionError::Error {
                message: "return cannot be used inside closures".to_owned(),
                labels: vec![Label::primary(
                    "return cannot be used inside closures",
                    span,
                )],
                notes: Vec::new(),
            },
            ExpressionError::Error {
                message,
                mut labels,
                notes,
            } => {
                labels.push(Label::primary(message.clone(), self.span));

                ExpressionError::Error {
                    message: format!(
                        r#"function call error for "{}" at ({}:{}): {}"#,
                        self.ident,
                        self.span.start(),
                        self.span.end(),
                        message
                    ),
                    labels,
                    notes,
                }
            }
        })
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        self.expr.resolve_constant(state)
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let mut state = state.clone();

        // TODO: functions with a closure do not correctly calculate type definitions
        // see: https://github.com/vectordotdev/vector/issues/13782

        // Evaluate arguments to correctly calculate any side-effects from them.
        // This doesn't actually match current runtime behavior in some cases,
        // but that will be changed.
        // see: https://github.com/vectordotdev/vector/issues/13752
        let arguments_may_fail_type_check =
            apply_argument_type_info(&self.arguments, &self.argument_parameters, &mut state);

        let mut expr_result = self.expr.apply_type_info(&mut state);

        // If one of the arguments only partially matches the function type
        // definition, then we mark the entire function as fallible.
        //
        // This allows for progressive type-checking, by handling any potential
        // type error the function throws, instead of having to enforce
        // exact-type invariants for individual arguments.
        //
        // That is, this program triggers the `InvalidArgumentKind` error:
        //
        //     slice(10, 1)
        //
        // This is because `slice` expects either a string or an array, but it
        // receives an integer. The concept of "progressive type checking" does
        // not apply in this case, because this call can never succeed.
        //
        // However, given these example events:
        //
        //     { "foo": "bar" }
        //     { "foo": 10.5 }
        //
        // If we were to run the same program, but against the `foo` field:
        //
        //     slice(.foo, 1)
        //
        // In this situation, progressive type checking _does_ make sense,
        // because we can't know at compile-time what the eventual value of
        // `.foo` will be. We mark `.foo` as "any", which includes the "array"
        // and "string" types, so the program can now be made infallible by
        // handling any potential type error the function returns:
        //
        //     slice(.foo, 1) ?? []
        //
        // Note that this rule doesn't just apply to "any" kind (in fact, "any"
        // isn't a kind, it's simply a term meaning "all possible VRL values"),
        // but it applies whenever there's an _intersection_ but not an exact
        // _match_ between two types.
        //
        // Here's another example to demonstrate this:
        //
        //     { "foo": "foobar" }
        //     { "foo": ["foo", "bar"] }
        //     { "foo": 10.5 }
        //
        //     foo = slice(.foo, 1) ?? .foo
        //     .foo = upcase(foo) ?? foo
        //
        // This would result in the following outcomes:
        //
        //     { "foo": "OOBAR" }
        //     { "foo": ["bar", "baz"] }
        //     { "foo": 10.5 }
        //
        // For the first event, both the `slice` and `upcase` functions succeed.
        // For the second event, only the `slice` function succeeds.
        // For the third event, both functions fail.
        //

        if arguments_may_fail_type_check {
            expr_result = expr_result.fallible();
        }

        // If the function has a closure attached, and that closure is fallible,
        // then the function call itself becomes fallible.
        //
        // Given that `FunctionClosure` also implements `Expression`, and
        // function implementations can access this closure, it is possible the
        // function implementation already handles potential closure
        // fallibility, but to be on the safe side, we ensure it is set properly
        // here.
        //
        // Note that, since closures are tied to function calls, it is still
        // possible to silence potential closure errors using the "abort on
        // error" function-call feature (see below).
        if self.closure_fallible {
            expr_result = expr_result.fallible();
        }

        if self.abort_on_error {
            expr_result = expr_result.infallible();
        }

        TypeInfo::new(state, expr_result)
    }
}

impl fmt::Display for FunctionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.ident.fmt(f)?;
        f.write_str("(")?;

        let arguments = self.arguments_fmt();
        let mut iter = arguments.iter().peekable();
        while let Some(arg) = iter.next() {
            f.write_str(arg)?;

            if iter.peek().is_some() {
                f.write_str(", ")?;
            }
        }

        f.write_str(")")
    }
}

impl fmt::Debug for FunctionCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FunctionCall(")?;
        self.ident.fmt(f)?;

        f.write_str("(")?;

        let arguments = self.arguments_dbg();
        let mut iter = arguments.iter().peekable();
        while let Some(arg) = iter.next() {
            f.write_str(arg)?;

            if iter.peek().is_some() {
                f.write_str(", ")?;
            }
        }

        f.write_str("))")
    }
}

impl PartialEq for FunctionCall {
    fn eq(&self, other: &Self) -> bool {
        self.ident == other.ident
    }
}

// -----------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub(crate) struct InvalidArgumentErrorContext {
    pub(crate) function_ident: &'static str,
    pub(crate) abort_on_error: bool,
    pub(crate) arguments_fmt: Vec<String>,
    pub(crate) parameter: Parameter,
    pub(crate) got: Kind,
    pub(crate) argument: FunctionArgument,
    pub(crate) argument_span: Span,
}

#[derive(thiserror::Error, Debug, Clone)]
pub(crate) enum Warning {
    #[error("can't abort infallible function")]
    AbortInfallible { ident_span: Span, abort_span: Span },
}

#[derive(thiserror::Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum FunctionCallError {
    #[error("call to undefined function")]
    Undefined {
        ident_span: Span,
        ident: Ident,
        idents: Vec<&'static str>,
    },

    #[error("wrong number of function arguments")]
    WrongNumberOfArgs { arguments_span: Span, max: usize },

    #[error("unknown function argument keyword")]
    UnknownKeyword {
        keyword_span: Span,
        ident_span: Span,
        keywords: Vec<&'static str>,
    },

    #[error("missing function argument")]
    MissingArgument {
        call_span: Span,
        keyword: &'static str,
        position: usize,
    },

    #[error("function compilation error: error[E{}] {}", error.code(), error)]
    Compilation {
        call_span: Span,
        error: Box<dyn DiagnosticMessage>,
    },

    #[error("invalid argument type")]
    InvalidArgumentKind(InvalidArgumentErrorContext),

    #[error("fallible argument")]
    FallibleArgument { expr_span: Span },

    #[error("unexpected closure")]
    UnexpectedClosure { call_span: Span, closure_span: Span },

    #[error("missing closure")]
    MissingClosure {
        call_span: Span,
        example: Option<Example>,
    },

    #[error("invalid closure arity")]
    ClosureArityMismatch {
        ident_span: Span,
        closure_arguments_span: Span,
        expected: usize,
        supplied: usize,
    },
    #[error("type mismatch in closure parameter")]
    ClosureParameterTypeMismatch { call_span: Span, found_kind: Kind },
    #[error("type mismatch in closure return type")]
    ReturnTypeMismatch {
        block_span: Span,
        found_kind: Kind,
        expected_kind: Kind,
    },
}

impl DiagnosticMessage for Warning {
    fn code(&self) -> usize {
        match self {
            AbortInfallible { .. } => codes::CompilerCode::AbortInfallible as usize,
        }
    }

    fn labels(&self) -> Vec<Label> {
        match self {
            AbortInfallible {
                ident_span,
                abort_span,
            } => {
                vec![
                    Label::primary("this function can't fail", ident_span),
                    Label::context("remove this abort (!) instruction", abort_span),
                ]
            }
        }
    }

    fn notes(&self) -> Vec<Note> {
        vec![SeeErrorDocs]
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }
}

impl DiagnosticMessage for FunctionCallError {
    fn code(&self) -> usize {
        use FunctionCallError::{
            ClosureArityMismatch, ClosureParameterTypeMismatch, Compilation, FallibleArgument,
            InvalidArgumentKind, MissingArgument, MissingClosure, ReturnTypeMismatch, Undefined,
            UnexpectedClosure, UnknownKeyword, WrongNumberOfArgs,
        };

        match self {
            Undefined { .. } => codes::ExprCode::UndefinedFunction as usize,
            WrongNumberOfArgs { .. } => codes::ExprCode::WrongNumberOfArgs as usize,
            UnknownKeyword { .. } => codes::ExprCode::UnknownKeyword as usize,
            Compilation { .. } => codes::CompilerCode::FunctionCompilation as usize,
            MissingArgument { .. } => codes::ExprCode::MissingArgument as usize,
            InvalidArgumentKind { .. } => codes::ExprCode::InvalidArgumentKind as usize,
            FallibleArgument { .. } => codes::CompilerCode::FallibleArgument as usize,
            UnexpectedClosure { .. } => codes::ExprCode::UnexpectedClosure as usize,
            MissingClosure { .. } => codes::ExprCode::MissingClosure as usize,
            ClosureArityMismatch { .. } => codes::ExprCode::ClosureArityMismatch as usize,
            ClosureParameterTypeMismatch { .. } => {
                codes::ExprCode::ClosureParameterTypeMismatch as usize
            }
            ReturnTypeMismatch { .. } => codes::ExprCode::ReturnTypeMismatch as usize,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn labels(&self) -> Vec<Label> {
        use FunctionCallError::{
            ClosureArityMismatch, ClosureParameterTypeMismatch, Compilation, FallibleArgument,
            InvalidArgumentKind, MissingArgument, MissingClosure, ReturnTypeMismatch, Undefined,
            UnexpectedClosure, UnknownKeyword, WrongNumberOfArgs,
        };

        match self {
            Undefined {
                ident_span,
                ident,
                idents,
            } => {
                let mut vec = vec![Label::primary("undefined function", ident_span)];
                let ident_chars = ident.as_ref().chars().collect::<Vec<_>>();

                if let Some((idx, _)) = idents
                    .iter()
                    .map(|possible| {
                        let possible_chars = possible.chars().collect::<Vec<_>>();
                        levenstein::distance(&ident_chars, &possible_chars)
                    })
                    .enumerate()
                    .min_by_key(|(_, score)| *score)
                {
                    {
                        let guessed: &str = idents[idx];
                        vec.push(Label::context(
                            format!(r#"did you mean "{guessed}"?"#),
                            ident_span,
                        ));
                    }
                }

                vec
            }

            WrongNumberOfArgs {
                arguments_span,
                max,
            } => {
                let arg = if *max == 1 { "argument" } else { "arguments" };

                vec![
                    Label::primary("too many function arguments", arguments_span),
                    Label::context(
                        format!("this function takes a maximum of {max} {arg}"),
                        arguments_span,
                    ),
                ]
            }

            UnknownKeyword {
                keyword_span,
                ident_span,
                keywords,
            } => vec![
                Label::primary("unknown keyword", keyword_span),
                Label::context(
                    format!(
                        "this function accepts the following keywords: {}",
                        keywords
                            .iter()
                            .map(|k| format!(r#""{k}""#))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    ident_span,
                ),
            ],

            Compilation { call_span, error } => error
                .labels()
                .into_iter()
                .map(|mut label| {
                    label.span = *call_span;
                    label
                })
                .collect(),

            MissingArgument {
                call_span,
                keyword,
                position,
            } => {
                vec![Label::primary(
                    format!(r#"required argument missing: "{keyword}" (position {position})"#),
                    call_span,
                )]
            }

            InvalidArgumentKind(context) => {
                let keyword = context.parameter.keyword;
                let expected = context.parameter.kind();
                let expr_span = context.argument.span();

                // TODO: extract this out into a helper
                let kind_str = |kind: &Kind| {
                    if kind.is_any() {
                        kind.to_string()
                    } else {
                        let array_kind = kind.as_array().and_then(|array| {
                            let element_kind = array.unknown_kind().without_undefined();
                            (!array.is_any()
                                && array.known().is_empty()
                                && element_kind.contains_any_defined())
                            .then(|| format!("array<{element_kind}>"))
                        });
                        let display = array_kind.map_or_else(
                            || kind.to_string(),
                            |array_kind| kind.to_string().replacen("array", &array_kind, 1),
                        );

                        if kind.is_exact() {
                            format!("the exact type {display}")
                        } else {
                            format!("one of {display}")
                        }
                    }
                };

                vec![
                    Label::primary(
                        format!("this expression resolves to {}", kind_str(&context.got)),
                        expr_span,
                    ),
                    Label::context(
                        format!(
                            r#"but the parameter "{}" expects {}"#,
                            keyword,
                            kind_str(&expected)
                        ),
                        context.argument_span,
                    ),
                ]
            }

            FallibleArgument { expr_span } => vec![
                Label::primary("this expression can fail", expr_span),
                Label::context(
                    "handle the error before passing it in as an argument",
                    expr_span,
                ),
            ],
            UnexpectedClosure {
                call_span,
                closure_span,
            } => vec![
                Label::primary("unexpected closure", closure_span),
                Label::context("this function does not accept a closure", call_span),
            ],
            MissingClosure { call_span, .. } => {
                vec![Label::primary("this function expects a closure", call_span)]
            }
            ClosureArityMismatch {
                ident_span,
                closure_arguments_span,
                expected,
                supplied,
            } => vec![
                Label::primary(
                    format!("this function requires a closure with {expected} argument(s)"),
                    ident_span,
                ),
                Label::context(
                    format!("but {supplied} argument(s) are supplied"),
                    closure_arguments_span,
                ),
            ],
            ClosureParameterTypeMismatch {
                call_span,
                found_kind,
            } => vec![
                Label::primary(
                    "the closure tied to this function call expects a different input value",
                    call_span,
                ),
                Label::context(
                    format!(
                        "expression has an inferred type of {found_kind} where an array or object was expected"
                    ),
                    call_span,
                ),
            ],
            ReturnTypeMismatch {
                block_span,
                found_kind,
                expected_kind,
            } => vec![
                Label::primary("block returns invalid value type", block_span),
                Label::context(format!("expected: {expected_kind}"), block_span),
                Label::context(format!("received: {found_kind}"), block_span),
            ],
        }
    }

    fn notes(&self) -> Vec<Note> {
        use FunctionCallError::{
            Compilation, FallibleArgument, InvalidArgumentKind, MissingClosure, WrongNumberOfArgs,
        };

        match self {
            WrongNumberOfArgs { .. } => vec![Note::SeeDocs(
                "function arguments".to_owned(),
                Urls::expression_docs_url("#arguments"),
            )],
            FallibleArgument { .. } => vec![Note::SeeErrorDocs],
            InvalidArgumentKind(context) => {
                // TODO: move this into a generic helper function
                let kind = &context.parameter.kind();
                let argument = &context.argument;
                let guard = if kind.is_bytes() {
                    format!("string!({argument})")
                } else if kind.is_integer() {
                    format!("int!({argument})")
                } else if kind.is_float() {
                    format!("float!({argument})")
                } else if kind.is_boolean() {
                    format!("bool!({argument})")
                } else if kind.is_object() {
                    format!("object!({argument})")
                } else if kind.is_array() {
                    if context.parameter.has_element_kind_constraint()
                        && context
                            .parameter
                            .kind_without_element_constraint()
                            .is_superset(&context.got)
                            .is_ok()
                    {
                        return vec![Note::SeeErrorDocs];
                    }
                    format!("array!({argument})")
                } else if kind.is_timestamp() {
                    format!("timestamp!({argument})")
                } else {
                    return vec![];
                };

                let coerce = if kind.is_bytes() {
                    Some(format!(r#"to_string({argument}) ?? "default""#))
                } else if kind.is_integer() {
                    Some(format!("to_int({argument}) ?? 0"))
                } else if kind.is_float() {
                    Some(format!("to_float({argument}) ?? 0"))
                } else if kind.is_boolean() {
                    Some(format!("to_bool({argument}) ?? false"))
                } else if kind.is_timestamp() {
                    Some(format!("to_unix_timestamp({argument}) ?? now()"))
                } else {
                    None
                };

                let args = {
                    let mut args = String::new();
                    let mut iter = context.arguments_fmt.iter().peekable();
                    while let Some(arg) = iter.next() {
                        args.push_str(arg);
                        if iter.peek().is_some() {
                            args.push_str(", ");
                        }
                    }

                    args
                };

                let abort = if context.abort_on_error { "!" } else { "" };

                let mut notes = vec![];

                let call = format!("{}{abort}({args})", context.function_ident);

                notes.append(&mut Note::solution(
                    "ensuring an appropriate type at runtime",
                    vec![format!("{argument} = {guard}"), call.clone()],
                ));

                if let Some(coerce) = coerce {
                    notes.append(&mut Note::solution(
                        "coercing to an appropriate type and specifying a default value as a fallback in case coercion fails",
                        vec![format!("{argument} = {coerce}"), call],
                    ));
                }

                notes.push(Note::SeeErrorDocs);

                notes
            }

            Compilation { error, .. } => error.notes(),

            MissingClosure { example, .. } if example.is_some() => {
                let code = example.unwrap().source.to_owned();
                vec![Note::Example(code)]
            }

            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        compiler::{Category, FunctionExpression, codes, value::kind},
        stdlib,
    };

    use super::*;

    #[derive(Clone, Debug)]
    struct Fn;

    impl FunctionExpression for Fn {
        fn resolve(&self, _ctx: &mut Context) -> Resolved {
            todo!()
        }

        fn type_def(&self, _state: &TypeState) -> TypeDef {
            TypeDef::null().infallible()
        }
    }

    #[derive(Debug)]
    struct TestFn;

    impl Function for TestFn {
        fn identifier(&self) -> &'static str {
            "test"
        }

        fn usage(&self) -> &'static str {
            "Test function"
        }

        fn category(&self) -> &'static str {
            Category::Debug.as_ref()
        }

        fn return_kind(&self) -> u16 {
            kind::NULL
        }

        fn examples(&self) -> &'static [crate::compiler::function::Example] {
            &[]
        }

        fn parameters(&self) -> &'static [Parameter] {
            const PARAMETERS: &[Parameter] = &[
                Parameter::optional("one", kind::INTEGER, "one"),
                Parameter::optional("two", kind::INTEGER, "two"),
                Parameter::optional("three", kind::INTEGER, "three"),
            ];

            PARAMETERS
        }

        fn compile(
            &self,
            _state: &TypeState,
            _ctx: &mut FunctionCompileContext,
            _arguments: ArgumentList,
        ) -> crate::compiler::function::Compiled {
            Ok(Fn.as_expr())
        }
    }

    fn create_node<T>(inner: T) -> Node<T> {
        Node::new(Span::new(0, 0), inner)
    }

    fn create_argument(ident: Option<&str>, value: i64) -> FunctionArgument {
        use crate::compiler::expression::{Expr, Literal};

        FunctionArgument::new(
            ident.map(|ident| create_node(Ident::new(ident))),
            create_node(Expr::Literal(Literal::Integer(value))),
        )
    }

    fn create_function_call(arguments: Vec<Node<FunctionArgument>>) -> FunctionCall {
        let mut state = TypeState::default();
        let original_state = state.clone();
        let mut config = CompileConfig::default();
        Builder::new(
            Span::new(0, 0),
            Node::new(Span::new(0, 0), Ident::new("test")),
            false,
            arguments,
            &[Box::new(TestFn) as _],
            &original_state,
            &mut state,
            None,
        )
        .unwrap()
        .compile(
            &original_state,
            &mut state,
            None,
            LocalEnv::default(),
            &mut config,
        )
        .unwrap()
        .function_call
    }

    #[test]
    fn unrelated_side_effect_does_not_defer_element_kind_error() {
        let Err(diagnostics) = crate::compiler::compile(
            r#"items = [1]; join(items, { foo = 1; "," }) ?? "fallback""#,
            &stdlib::all(),
        ) else {
            panic!("invalid array element kind should fail compilation");
        };

        assert_eq!(
            diagnostics.errors()[0].code,
            codes::ExprCode::InvalidArgumentKind as usize
        );
    }

    #[test]
    fn later_parameter_side_effect_does_not_defer_element_kind_error() {
        let Err(diagnostics) = crate::compiler::compile(
            r#"items = [1]; contains_all(substrings: items, case_sensitive: { items = ["x"]; true }, value: "x") ?? false"#,
            &stdlib::all(),
        ) else {
            panic!("a later parameter cannot change an earlier argument");
        };

        assert_eq!(
            diagnostics.errors()[0].code,
            codes::ExprCode::InvalidArgumentKind as usize
        );
    }

    #[test]
    fn unconstrained_function_preserves_source_order_type_effects() {
        let Err(diagnostics) = crate::compiler::compile(
            r#"x = 0; slice!(start: { x = "ok"; 0 }, value: { x = 1; "abc" }); upcase(x)"#,
            &stdlib::all(),
        ) else {
            panic!("unconstrained calls must preserve source-order type effects");
        };

        assert_eq!(
            diagnostics.errors()[0].code,
            codes::ExprCode::InvalidArgumentKind as usize
        );
    }

    #[test]
    fn resolve_arguments_simple() {
        let call = create_function_call(vec![
            create_node(create_argument(None, 1)),
            create_node(create_argument(None, 2)),
            create_node(create_argument(None, 3)),
        ]);

        let params = call.resolve_arguments(&TestFn);
        let expected: Vec<(&'static str, Option<FunctionArgument>)> = vec![
            ("one", Some(create_argument(None, 1))),
            ("two", Some(create_argument(None, 2))),
            ("three", Some(create_argument(None, 3))),
        ];

        assert_eq!(Ok(expected), params);
    }

    #[test]
    fn resolve_arguments_named() {
        let call = create_function_call(vec![
            create_node(create_argument(Some("one"), 1)),
            create_node(create_argument(Some("two"), 2)),
            create_node(create_argument(Some("three"), 3)),
        ]);

        let params = call.resolve_arguments(&TestFn);
        let expected: Vec<(&'static str, Option<FunctionArgument>)> = vec![
            ("one", Some(create_argument(Some("one"), 1))),
            ("two", Some(create_argument(Some("two"), 2))),
            ("three", Some(create_argument(Some("three"), 3))),
        ];

        assert_eq!(Ok(expected), params);
    }

    #[test]
    fn resolve_arguments_named_unordered() {
        let call = create_function_call(vec![
            create_node(create_argument(Some("three"), 3)),
            create_node(create_argument(Some("two"), 2)),
            create_node(create_argument(Some("one"), 1)),
        ]);

        let params = call.resolve_arguments(&TestFn);
        let expected: Vec<(&'static str, Option<FunctionArgument>)> = vec![
            ("one", Some(create_argument(Some("one"), 1))),
            ("two", Some(create_argument(Some("two"), 2))),
            ("three", Some(create_argument(Some("three"), 3))),
        ];

        assert_eq!(Ok(expected), params);
    }

    #[test]
    fn resolve_arguments_unnamed_unordered_one() {
        let call = create_function_call(vec![
            create_node(create_argument(Some("three"), 3)),
            create_node(create_argument(None, 2)),
            create_node(create_argument(Some("one"), 1)),
        ]);

        let params = call.resolve_arguments(&TestFn);
        let expected: Vec<(&'static str, Option<FunctionArgument>)> = vec![
            ("one", Some(create_argument(Some("one"), 1))),
            ("two", Some(create_argument(None, 2))),
            ("three", Some(create_argument(Some("three"), 3))),
        ];

        assert_eq!(Ok(expected), params);
    }

    #[test]
    fn resolve_arguments_unnamed_unordered_two() {
        let call = create_function_call(vec![
            create_node(create_argument(Some("three"), 3)),
            create_node(create_argument(None, 1)),
            create_node(create_argument(None, 2)),
        ]);

        let params = call.resolve_arguments(&TestFn);
        let expected: Vec<(&'static str, Option<FunctionArgument>)> = vec![
            ("one", Some(create_argument(None, 1))),
            ("two", Some(create_argument(None, 2))),
            ("three", Some(create_argument(Some("three"), 3))),
        ];

        assert_eq!(Ok(expected), params);
    }
}
