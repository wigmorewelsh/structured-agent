mod duplicate_injections;
mod empty_blocks;
mod empty_functions;
mod infinite_loops;
mod placeholder_overuse;
mod unreachable_code;
mod unused_variables;

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

pub use duplicate_injections::DuplicateInjectionAnalyzer;
pub use empty_blocks::EmptyBlockAnalyzer;
pub use empty_functions::EmptyFunctionAnalyzer;
pub use infinite_loops::InfiniteLoopAnalyzer;
pub use placeholder_overuse::PlaceholderOveruseAnalyzer;
pub use unreachable_code::ReachabilityAnalyzer;
pub use unused_variables::UnusedVariableAnalyzer;

use crate::ast::Module;
use crate::types::{FileId, Span};
use codespan_reporting::diagnostic::Diagnostic;

pub trait Analyzer {
    fn name(&self) -> &str;
    fn analyze_module(&mut self, module: &Module, file_id: FileId) -> Vec<Warning>;
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
