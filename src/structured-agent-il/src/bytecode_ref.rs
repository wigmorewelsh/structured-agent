use std::collections::HashMap;
use std::fmt;

use structured_agent_runtime::symbols::{BodyRef, DefinitionPath};
use structured_agent_runtime::{Parameter, Type};

use crate::Instruction;
use crate::slot::{SlotKind, SlotTable};

#[derive(Clone, Debug)]
pub struct BytecodeRef {
    pub instructions: Vec<Instruction>,
    pub labels: HashMap<String, usize>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub documentation: Option<String>,
    pub slot_table: SlotTable,
}

impl BodyRef for BytecodeRef {}

#[derive(Clone, Debug)]
pub struct CompiledFunction {
    pub name: DefinitionPath,
    pub module_name: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: Type,
    pub instructions: Vec<Instruction>,
    pub labels: HashMap<String, usize>,
    pub documentation: Option<String>,
    pub slot_table: SlotTable,
}

impl fmt::Display for CompiledFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {}(", self.name)?;
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                writeln!(f, ",")?;
            }
            write!(f, "    {}: {}", param.name, param.param_type.name())?;
        }
        writeln!(f, "\n): {} {{", self.return_type.name())?;

        writeln!(f, "  .slots:")?;
        for info in self.slot_table.iter() {
            let kind = match info.kind {
                SlotKind::ReturnSlot => "ret  ",
                SlotKind::ValueParam => "param",
                SlotKind::Local => "local",
                SlotKind::Temp => "temp ",
            };
            writeln!(f, "    {:<4}  {}  {}", info.slot, kind, info.name)?;
        }

        let mut label_positions: Vec<(usize, &str)> = self
            .labels
            .iter()
            .map(|(name, pos)| (*pos, name.as_str()))
            .collect();
        label_positions.sort_by_key(|(pos, _)| *pos);

        let mut label_iter = label_positions.iter().peekable();

        for (i, instr) in self.instructions.iter().enumerate() {
            while let Some((pos, name)) = label_iter.peek() {
                if *pos == i {
                    writeln!(f, "  {}:", name)?;
                    label_iter.next();
                } else {
                    break;
                }
            }
            writeln!(
                f,
                "    {:3}: {}",
                i,
                instr.display_with_table(&self.slot_table)
            )?;
        }

        writeln!(f, "}}")?;
        Ok(())
    }
}
