mod branch_target;
mod call_arity;
mod context_balance;
mod double_drop;
mod duplicate_decl;
mod return_coverage;
mod unreachable_instructions;
mod variable_allocation;
mod variable_drop;

use crate::bytecode::{BytecodeRef, Instruction};
use crate::types::FileId;
use codespan_reporting::diagnostic::Diagnostic;

#[cfg(test)]
mod branch_target_test;

#[cfg(test)]
mod call_arity_test;

#[cfg(test)]
mod context_balance_test;

#[cfg(test)]
mod double_drop_test;

#[cfg(test)]
mod duplicate_decl_test;

#[cfg(test)]
mod return_coverage_test;

#[cfg(test)]
mod unreachable_instructions_test;

#[cfg(test)]
mod variable_allocation_test;

#[cfg(test)]
mod variable_drop_test;

pub use branch_target::BranchTargetAnalyzer;
pub use call_arity::CallArityAnalyzer;
pub use context_balance::ContextBalanceAnalyzer;
pub use double_drop::DoubleDropAnalyzer;
pub use duplicate_decl::DuplicateDeclAnalyzer;
pub use return_coverage::ReturnCoverageAnalyzer;
pub use unreachable_instructions::UnreachableInstructionAnalyzer;
pub use variable_allocation::VariableAllocationAnalyzer;
pub use variable_drop::VariableDropAnalyzer;

pub(crate) fn instruction_reads(instruction: &Instruction) -> Vec<&str> {
    match instruction {
        Instruction::Drop { name } => vec![name.as_str()],
        Instruction::Mov { src, .. } => vec![src.as_str()],
        Instruction::BrFalse { var, .. } => vec![var.as_str()],
        Instruction::BrTrue { var, .. } => vec![var.as_str()],
        Instruction::Switch { var, .. } => vec![var.as_str()],
        Instruction::Ret { var } => vec![var.as_str()],
        Instruction::CallBytecode { params, .. } | Instruction::CallExternal { params, .. } => {
            params.iter().map(String::as_str).collect()
        }
        Instruction::CtxEvent { var } => vec![var.as_str()],
        Instruction::ListCreate { elements, .. } => elements.iter().map(String::as_str).collect(),
        Instruction::LlmSelect { metadata_vars, .. } => {
            metadata_vars.iter().map(String::as_str).collect()
        }
        Instruction::StructNew { fields, .. } => {
            fields.iter().map(|(_, src)| src.as_str()).collect()
        }
        Instruction::StructGet { src, .. } => vec![src.as_str()],
        _ => vec![],
    }
}

pub(crate) fn instruction_writes(instruction: &Instruction) -> Option<&str> {
    match instruction {
        Instruction::Decl { name } => Some(name.as_str()),
        Instruction::LdcStr { dest, .. } => Some(dest.as_str()),
        Instruction::LdcBool { dest, .. } => Some(dest.as_str()),
        Instruction::LdcInt { dest, .. } => Some(dest.as_str()),
        Instruction::LdcUnit { dest } => Some(dest.as_str()),
        Instruction::Mov { dest, .. } => Some(dest.as_str()),
        Instruction::CallBytecode { dest, .. } | Instruction::CallExternal { dest, .. } => {
            Some(dest.as_str())
        }
        Instruction::MetaFunction { dest, .. } => Some(dest.as_str()),
        Instruction::ListCreate { dest, .. } => Some(dest.as_str()),
        Instruction::LlmPlaceholder { dest, .. } => Some(dest.as_str()),
        Instruction::LlmSelect { dest, .. } => Some(dest.as_str()),
        Instruction::LlmGenerate { dest, .. } => Some(dest.as_str()),
        Instruction::StructNew { dest, .. } => Some(dest.as_str()),
        Instruction::StructGet { dest, .. } => Some(dest.as_str()),
        _ => None,
    }
}

pub trait IlAnalyzer {
    fn name(&self) -> &str;
    fn analyze_function(&mut self, function: &BytecodeRef) -> Vec<IlWarning>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum IlWarning {
    VariableUsedBeforeAllocation {
        name: String,
        instruction_index: usize,
    },
    VariableNotDropped {
        name: String,
    },
    InvalidBranchTarget {
        instruction_index: usize,
        target: i32,
    },
    NoReturnPath,
    ContextUnderflow {
        instruction_index: usize,
    },
    ContextNotRestored {
        depth: i32,
    },
    UnreachableInstruction {
        instruction_index: usize,
    },
    DuplicateDeclaration {
        name: String,
        instruction_index: usize,
    },
    DoubleDrop {
        name: String,
        instruction_index: usize,
    },
    CallArityMismatch {
        function_name: String,
        expected: usize,
        got: usize,
        instruction_index: usize,
    },
}

impl IlWarning {
    pub fn message(&self) -> String {
        match self {
            IlWarning::VariableUsedBeforeAllocation {
                name,
                instruction_index,
            } => format!(
                "variable `{}` used at instruction {} before being allocated",
                name, instruction_index
            ),
            IlWarning::VariableNotDropped { name } => {
                format!("variable `{}` is allocated but never dropped", name)
            }
            IlWarning::InvalidBranchTarget {
                instruction_index,
                target,
            } => format!(
                "instruction {} branches to invalid target {}",
                instruction_index, target
            ),
            IlWarning::NoReturnPath => "function has no return instruction".to_string(),
            IlWarning::ContextUnderflow { instruction_index } => format!(
                "ctx.restore at instruction {} has no matching ctx.child",
                instruction_index
            ),
            IlWarning::ContextNotRestored { depth } => {
                format!("function exits with {} unclosed context(s)", depth)
            }
            IlWarning::UnreachableInstruction { instruction_index } => {
                format!("instruction {} is unreachable", instruction_index)
            }
            IlWarning::DuplicateDeclaration {
                name,
                instruction_index,
            } => format!(
                "variable `{}` declared a second time at instruction {}",
                name, instruction_index
            ),
            IlWarning::DoubleDrop {
                name,
                instruction_index,
            } => format!(
                "variable `{}` dropped a second time at instruction {}",
                name, instruction_index
            ),
            IlWarning::CallArityMismatch {
                function_name,
                expected,
                got,
                instruction_index,
            } => format!(
                "call to `{}` at instruction {} provides {} argument(s) but the function expects {}",
                function_name, instruction_index, got, expected
            ),
        }
    }

    pub fn to_diagnostic(&self) -> Diagnostic<FileId> {
        Diagnostic::warning().with_message(self.message())
    }
}

pub struct IlAnalysisRunner {
    analyzers: Vec<Box<dyn IlAnalyzer>>,
}

impl IlAnalysisRunner {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    pub fn with_analyzer(mut self, analyzer: Box<dyn IlAnalyzer>) -> Self {
        self.analyzers.push(analyzer);
        self
    }

    pub fn run(&mut self, function: &BytecodeRef) -> Vec<IlWarning> {
        let mut all_warnings = Vec::new();
        for analyzer in &mut self.analyzers {
            all_warnings.extend(analyzer.analyze_function(function));
        }
        all_warnings
    }
}

impl Default for IlAnalysisRunner {
    fn default() -> Self {
        Self::new()
    }
}
