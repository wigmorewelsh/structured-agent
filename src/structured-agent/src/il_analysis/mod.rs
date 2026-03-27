mod variable_allocation;
mod variable_drop;

use crate::bytecode::Instruction;

#[cfg(test)]
mod variable_allocation_test;

#[cfg(test)]
mod variable_drop_test;

pub use variable_allocation::VariableAllocationAnalyzer;
pub use variable_drop::VariableDropAnalyzer;

use crate::bytecode::CompiledFunction;

pub(crate) fn instruction_reads(instruction: &Instruction) -> Vec<&str> {
    match instruction {
        Instruction::Drop { name } => vec![name.as_str()],
        Instruction::Mov { src, .. } => vec![src.as_str()],
        Instruction::BrFalse { var, .. } => vec![var.as_str()],
        Instruction::BrTrue { var, .. } => vec![var.as_str()],
        Instruction::Switch { var, .. } => vec![var.as_str()],
        Instruction::Ret { var } => vec![var.as_str()],
        Instruction::Call { params, .. } => params.iter().map(String::as_str).collect(),
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
        Instruction::Call { dest, .. } => Some(dest.as_str()),
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
    fn analyze_function(&mut self, function: &CompiledFunction) -> Vec<IlWarning>;
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
        }
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

    pub fn run(&mut self, function: &CompiledFunction) -> Vec<IlWarning> {
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
