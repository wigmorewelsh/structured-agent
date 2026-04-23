use crate::Instruction;
use crate::slot::{Slot, SlotKind, SlotTable};
use std::collections::HashMap;

pub struct InstructionBuilder {
    instructions: Vec<Instruction>,
    labels: HashMap<String, usize>,
    pending_labels: Vec<(usize, String, PendingJumpKind)>,
    temp_counter: u32,
    label_counter: u32,
    slot_table: SlotTable,
}

enum PendingJumpKind {
    Br,
    BrFalse(Slot),
    SwitchCase(Slot, usize),
}

impl InstructionBuilder {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            labels: HashMap::new(),
            pending_labels: Vec::new(),
            temp_counter: 0,
            label_counter: 0,
            slot_table: SlotTable::new(),
        }
    }

    pub fn emit(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
    }

    pub fn emit_label(&mut self, label: &str) {
        let position = self.instructions.len();
        self.labels.insert(label.to_string(), position);
    }

    pub fn emit_br(&mut self, label: &str) {
        let position = self.instructions.len();
        self.pending_labels
            .push((position, label.to_string(), PendingJumpKind::Br));
        self.instructions.push(Instruction::Br { offset: 0 });
    }

    pub fn emit_brfalse(&mut self, var: Slot, label: &str) {
        let position = self.instructions.len();
        self.pending_labels
            .push((position, label.to_string(), PendingJumpKind::BrFalse(var)));
        self.instructions
            .push(Instruction::BrFalse { var, offset: 0 });
    }

    pub fn emit_switch(&mut self, var: Slot, case_labels: Vec<String>) {
        let position = self.instructions.len();
        for (index, label) in case_labels.iter().enumerate() {
            self.pending_labels.push((
                position,
                label.clone(),
                PendingJumpKind::SwitchCase(var, index),
            ));
        }
        let offsets = vec![0; case_labels.len()];
        self.instructions.push(Instruction::Switch { var, offsets });
    }

    pub fn next_temp_slot(&mut self) -> Slot {
        let idx = self.temp_counter;
        self.temp_counter += 1;
        self.alloc_slot(SlotKind::Temp, format!("$t{}", idx))
    }

    pub fn next_label_id(&mut self) -> u32 {
        let id = self.label_counter;
        self.label_counter += 1;
        id
    }

    pub fn alloc_slot(&mut self, kind: SlotKind, name: impl Into<String>) -> Slot {
        self.slot_table.push(kind, name)
    }

    pub fn build(
        mut self,
    ) -> Result<(Vec<Instruction>, HashMap<String, usize>, SlotTable), String> {
        for (position, label, kind) in self.pending_labels {
            let target = self
                .labels
                .get(&label)
                .ok_or_else(|| format!("Undefined label: {}", label))?;

            let target_position = *target as i32;

            match kind {
                PendingJumpKind::Br => {
                    self.instructions[position] = Instruction::Br {
                        offset: target_position,
                    };
                }
                PendingJumpKind::BrFalse(var) => {
                    self.instructions[position] = Instruction::BrFalse {
                        var,
                        offset: target_position,
                    };
                }
                PendingJumpKind::SwitchCase(var, index) => {
                    if let Instruction::Switch {
                        var: switch_var,
                        offsets,
                    } = &mut self.instructions[position]
                        && *switch_var == var
                    {
                        offsets[index] = target_position;
                    }
                }
            }
        }

        Ok((self.instructions, self.labels, self.slot_table))
    }
}

impl Default for InstructionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
