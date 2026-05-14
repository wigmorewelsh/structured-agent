mod constant_conditions;
mod duplicate_injections;
mod empty_blocks;
mod empty_functions;
mod infinite_loops;
mod missing_return_or_injection;
mod orphaned_injection;
mod overwritten_values;
mod placeholder_overuse;
mod redundant_select;
mod unreachable_code;
mod unused_expressions;
mod unused_return_values;
mod unused_variables;
mod variable_shadowing;

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod empty_blocks_test;

#[cfg(test)]
mod empty_functions_test;

#[cfg(test)]
mod duplicate_injections_test;

#[cfg(test)]
mod placeholder_overuse_test;

#[cfg(test)]
mod redundant_select_test;

#[cfg(test)]
mod constant_conditions_test;

#[cfg(test)]
mod variable_shadowing_test;

#[cfg(test)]
mod overwritten_values_test;

#[cfg(test)]
mod unused_expressions_test;

#[cfg(test)]
mod unused_return_values_test;

#[cfg(test)]
mod missing_return_or_injection_test;

#[cfg(test)]
mod orphaned_injection_test;

#[cfg(test)]
mod unused_variables_test;

pub use constant_conditions::ConstantConditionAnalyzer;
pub use duplicate_injections::DuplicateInjectionAnalyzer;
pub use empty_blocks::EmptyBlockAnalyzer;
pub use empty_functions::EmptyFunctionAnalyzer;
pub use infinite_loops::InfiniteLoopAnalyzer;
pub use missing_return_or_injection::MissingReturnOrInjectionAnalyzer;
pub use orphaned_injection::OrphanedInjectionAnalyzer;
pub use overwritten_values::OverwrittenValueAnalyzer;
pub use placeholder_overuse::PlaceholderOveruseAnalyzer;
pub use redundant_select::RedundantSelectAnalyzer;
pub use unreachable_code::ReachabilityAnalyzer;
pub use unused_expressions::UnusedExpressionAnalyzer;
pub use unused_return_values::UnusedReturnValueAnalyzer;
pub use unused_variables::UnusedVariableAnalyzer;
pub use variable_shadowing::VariableShadowingAnalyzer;

use codespan_reporting::diagnostic::Diagnostic;
use structured_agent_ast::ast::{Definition, Function, Module};
use structured_agent_ast::types::{FileId, Span};

pub trait Analyzer {
    fn name(&self) -> &str;
    fn analyze_function(&mut self, _func: &Function, _file_id: FileId) -> Vec<Warning> {
        vec![]
    }
    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut warnings = Vec::new();
        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                warnings.extend(self.analyze_function(func, file_id));
            }
        }
        warnings
    }
}

#[derive(Debug, Clone)]
pub enum Warning {
    UnusedVariable {
        name: String,
        span: Span,
        file_id: FileId,
    },
    UnreachableCode {
        span: Span,
        file_id: FileId,
    },
    PotentialInfiniteLoop {
        span: Span,
        file_id: FileId,
    },
    EmptyBlock {
        block_type: String,
        span: Span,
        file_id: FileId,
    },
    EmptyFunction {
        name: String,
        span: Span,
        file_id: FileId,
    },
    DuplicateInjection {
        span: Span,
        file_id: FileId,
    },
    PlaceholderOveruse {
        placeholder_count: usize,
        span: Span,
        file_id: FileId,
    },
    RedundantSelect {
        span: Span,
        file_id: FileId,
    },
    ConstantCondition {
        condition_value: bool,
        span: Span,
        file_id: FileId,
    },
    VariableShadowing {
        name: String,
        inner_span: Span,
        outer_span: Span,
        file_id: FileId,
    },
    OverwrittenValue {
        name: String,
        span: Span,
        file_id: FileId,
    },
    UnusedReturnValue {
        function_name: String,
        span: Span,
        file_id: FileId,
    },
    UnusedExpression {
        span: Span,
        file_id: FileId,
    },
    MissingReturnOrInjection {
        name: String,
        span: Span,
        file_id: FileId,
    },
    OrphanedInjection {
        span: Span,
        file_id: FileId,
    },
}

impl Warning {
    pub fn to_diagnostic(&self) -> Diagnostic<FileId> {
        use codespan_reporting::diagnostic::Label;

        match self {
            Warning::UnusedVariable {
                name,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!("unused variable `{}`", name))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("variable declared but never read"),
                ]),
            Warning::UnreachableCode { span, file_id } => Diagnostic::warning()
                .with_message("unreachable code")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("this code will never execute"),
                ]),
            Warning::PotentialInfiniteLoop { span, file_id } => Diagnostic::warning()
                .with_message("potential infinite loop")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("loop condition is always true"),
                ]),
            Warning::EmptyBlock {
                block_type,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!("empty {} block", block_type))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("block contains no statements"),
                ]),
            Warning::EmptyFunction {
                name,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!("function `{}` has empty body", name))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("function contains no statements"),
                ]),
            Warning::DuplicateInjection { span, file_id } => Diagnostic::warning()
                .with_message("duplicate consecutive injection")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("identical injection appears consecutively"),
                ]),
            Warning::PlaceholderOveruse {
                placeholder_count,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message("function call uses only placeholders")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range()).with_message(format!(
                        "all {} arguments are placeholders",
                        placeholder_count
                    )),
                ]),
            Warning::RedundantSelect { span, file_id } => Diagnostic::warning()
                .with_message("select statement with only one branch")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("consider using direct assignment instead"),
                ]),
            Warning::ConstantCondition {
                condition_value,
                span,
                file_id,
            } => {
                let msg = if *condition_value {
                    "if statement condition is always true"
                } else {
                    "if statement condition is always false"
                };
                Diagnostic::warning()
                    .with_message("if statement has constant condition")
                    .with_labels(vec![
                        Label::primary(*file_id, span.to_byte_range()).with_message(msg),
                    ])
            }
            Warning::VariableShadowing {
                name,
                inner_span,
                outer_span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!("variable `{}` shadows outer declaration", name))
                .with_labels(vec![
                    Label::primary(*file_id, inner_span.to_byte_range())
                        .with_message("inner variable declared here"),
                    Label::secondary(*file_id, outer_span.to_byte_range())
                        .with_message("outer variable declared here"),
                ]),
            Warning::OverwrittenValue {
                name,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!(
                    "value assigned to `{}` is overwritten before being read",
                    name
                ))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range()).with_message("value never read"),
                ]),
            Warning::UnusedReturnValue {
                function_name,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!("return value of `{}` is ignored", function_name))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("return value not used"),
                ]),
            Warning::UnusedExpression { span, file_id } => Diagnostic::warning()
                .with_message("unused expression")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("expression result is not used; add `!` to inject it"),
                ]),
            Warning::MissingReturnOrInjection {
                name,
                span,
                file_id,
            } => Diagnostic::warning()
                .with_message(format!(
                    "function `{}` has no return statement or injection",
                    name
                ))
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range())
                        .with_message("function has no return or injection"),
                ]),
            Warning::OrphanedInjection { span, file_id } => Diagnostic::warning()
                .with_message("orphaned injection")
                .with_labels(vec![
                    Label::primary(*file_id, span.to_byte_range()).with_message(
                        "injection at end of block has no effect; nothing in this scope will consume it",
                    ),
                ]),
        }
    }
}

pub struct AnalysisRunner {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalysisRunner {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    pub fn with_analyzer(mut self, analyzer: Box<dyn Analyzer>) -> Self {
        self.analyzers.push(analyzer);
        self
    }

    pub fn run(&mut self, module: &Module, file_id: FileId) -> Vec<Warning> {
        let mut all_warnings = Vec::new();
        for analyzer in &mut self.analyzers {
            all_warnings.extend(analyzer.analyze_module(module, file_id));
        }
        all_warnings
    }
}

impl Default for AnalysisRunner {
    fn default() -> Self {
        Self::new()
    }
}
