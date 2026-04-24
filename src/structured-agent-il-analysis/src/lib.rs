mod branch_target;
mod call_arity;
mod context_balance;
mod return_coverage;
mod unreachable_instructions;
mod variable_allocation;

use codespan_reporting::diagnostic::Diagnostic;
use structured_agent_ast::types::FileId;
use structured_agent_il::slot::Slot;
use structured_agent_il::{BytecodeRef, Instruction};

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
mod branch_target_test;

#[cfg(test)]
mod call_arity_test;

#[cfg(test)]
mod context_balance_test;

#[cfg(test)]
mod return_coverage_test;

#[cfg(test)]
mod unreachable_instructions_test;

#[cfg(test)]
mod variable_allocation_test;

pub use branch_target::BranchTargetAnalyzer;
pub use call_arity::CallArityAnalyzer;
pub use context_balance::ContextBalanceAnalyzer;
pub use return_coverage::ReturnCoverageAnalyzer;
pub use unreachable_instructions::UnreachableInstructionAnalyzer;
pub use variable_allocation::VariableAllocationAnalyzer;

pub fn instruction_reads(instruction: &Instruction) -> Vec<Slot> {
    match instruction {
        Instruction::Mov { src, .. } => vec![*src],
        Instruction::BrFalse { var, .. } => vec![*var],
        Instruction::BrTrue { var, .. } => vec![*var],
        Instruction::Switch { var, .. } => vec![*var],
        Instruction::Ret { var } => vec![*var],
        Instruction::CallBytecode { params, .. } | Instruction::CallExternal { params, .. } => {
            params.clone()
        }
        Instruction::CtxEvent { var } => vec![*var],
        Instruction::ListCreate { elements, .. } => elements.clone(),
        Instruction::LlmSelect { metadata_vars, .. } => metadata_vars.clone(),
        Instruction::StructNew { fields, .. } => fields.iter().map(|(_, src)| *src).collect(),
        Instruction::StructGet { src, .. } => vec![*src],
        Instruction::CallIndirect { params, .. } => params.clone(),
        Instruction::LoadModule { params, .. } => params.clone(),
        _ => vec![],
    }
}

pub fn instruction_writes(instruction: &Instruction) -> Option<Slot> {
    match instruction {
        Instruction::LdcStr { dest, .. } => Some(*dest),
        Instruction::LdcBool { dest, .. } => Some(*dest),
        Instruction::LdcInt { dest, .. } => Some(*dest),
        Instruction::LdcUnit { dest } => Some(*dest),
        Instruction::Mov { dest, .. } => Some(*dest),
        Instruction::CallBytecode { dest, .. } | Instruction::CallExternal { dest, .. } => {
            Some(*dest)
        }
        Instruction::LoadModule { dest, .. } => Some(*dest),
        Instruction::CallIndirect { dest, .. } => Some(*dest),
        Instruction::MetaFunction { dest, .. } => Some(*dest),
        Instruction::ListCreate { dest, .. } => Some(*dest),
        Instruction::LlmPlaceholder { dest, .. } => Some(*dest),
        Instruction::LlmSelect { dest, .. } => Some(*dest),
        Instruction::LlmGenerate { dest, .. } => Some(*dest),
        Instruction::StructNew { dest, .. } => Some(*dest),
        Instruction::StructGet { dest, .. } => Some(*dest),
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
