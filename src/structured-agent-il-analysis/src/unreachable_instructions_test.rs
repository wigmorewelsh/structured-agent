#[cfg(test)]
mod tests {
    use crate::{IlAnalyzer, IlWarning, UnreachableInstructionAnalyzer};
    use structured_agent_il::{Instruction, Slot};

    use super::super::test_helpers::make_function;

    #[test]
    fn no_warning_for_normal_function() {
        let instructions = vec![
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_on_instruction_after_ret() {
        let instructions = vec![
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Ret { var: Slot(0) },
            Instruction::LdcUnit { dest: Slot(1) },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::UnreachableInstruction {
                instruction_index: 2,
            }
        );
    }

    #[test]
    fn warns_on_instruction_after_unconditional_br() {
        let instructions = vec![
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Br { offset: 0 },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::UnreachableInstruction {
                instruction_index: 2,
            }
        );
    }

    #[test]
    fn no_warning_when_instruction_after_br_is_a_jump_target() {
        let instructions = vec![
            Instruction::LdcBool {
                dest: Slot(0),
                value: false,
            },
            Instruction::BrFalse {
                var: Slot(0),
                offset: 3,
            },
            Instruction::Br { offset: 3 },
            Instruction::LdcUnit { dest: Slot(1) },
            Instruction::Ret { var: Slot(1) },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn conditional_branch_does_not_make_next_instruction_unreachable() {
        let instructions = vec![
            Instruction::LdcBool {
                dest: Slot(0),
                value: true,
            },
            Instruction::BrFalse {
                var: Slot(0),
                offset: 3,
            },
            Instruction::LdcUnit { dest: Slot(1) },
            Instruction::Ret { var: Slot(1) },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
