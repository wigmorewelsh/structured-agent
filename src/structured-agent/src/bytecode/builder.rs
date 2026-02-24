use super::Instruction;
use std::collections::HashMap;

pub struct InstructionBuilder {
    instructions: Vec<Instruction>,
    labels: HashMap<String, usize>,
    pending_labels: Vec<(usize, String, PendingJumpKind)>,
    temp_counter: usize,
}

enum PendingJumpKind {
    Br,
    BrFalse(String),
    BrTrue(String),
    SwitchCase(String, usize),
}

impl InstructionBuilder {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            labels: HashMap::new(),
            pending_labels: Vec::new(),
            temp_counter: 0,
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

    pub fn emit_brfalse(&mut self, var: String, label: &str) {
        let position = self.instructions.len();
        self.pending_labels.push((
            position,
            label.to_string(),
            PendingJumpKind::BrFalse(var.clone()),
        ));
        self.instructions
            .push(Instruction::BrFalse { var, offset: 0 });
    }

    pub fn emit_brtrue(&mut self, var: String, label: &str) {
        let position = self.instructions.len();
        self.pending_labels.push((
            position,
            label.to_string(),
            PendingJumpKind::BrTrue(var.clone()),
        ));
        self.instructions
            .push(Instruction::BrTrue { var, offset: 0 });
    }

    pub fn emit_switch(&mut self, var: String, case_labels: Vec<String>) {
        let position = self.instructions.len();
        for (index, label) in case_labels.iter().enumerate() {
            self.pending_labels.push((
                position,
                label.clone(),
                PendingJumpKind::SwitchCase(var.clone(), index),
            ));
        }
        let offsets = vec![0; case_labels.len()];
        self.instructions.push(Instruction::Switch { var, offsets });
    }

    pub fn next_temp(&mut self) -> String {
        let temp = format!("$tmp{}", self.temp_counter);
        self.temp_counter += 1;
        temp
    }

    pub fn build(mut self) -> Result<Vec<Instruction>, String> {
        for (position, label, kind) in self.pending_labels {
            let target = self
                .labels
                .get(&label)
                .ok_or_else(|| format!("Undefined label: {}", label))?;

            let offset = (*target as i32) - (position as i32);

            match kind {
                PendingJumpKind::Br => {
                    self.instructions[position] = Instruction::Br { offset };
                }
                PendingJumpKind::BrFalse(var) => {
                    self.instructions[position] = Instruction::BrFalse { var, offset };
                }
                PendingJumpKind::BrTrue(var) => {
                    self.instructions[position] = Instruction::BrTrue { var, offset };
                }
                PendingJumpKind::SwitchCase(var, index) => {
                    if let Instruction::Switch {
                        var: switch_var,
                        offsets,
                    } = &mut self.instructions[position]
                    {
                        if switch_var == &var {
                            offsets[index] = offset;
                        }
                    }
                }
            }
        }

        Ok(self.instructions)
    }
}

impl Default for InstructionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
