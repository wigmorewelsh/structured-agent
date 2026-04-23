#[cfg(test)]
mod tests {
    use crate::{BranchTargetAnalyzer, IlAnalyzer, IlWarning};
    use structured_agent_il::{Instruction, Slot};

    use super::super::test_helpers::make_function;

    #[test]
    fn no_warning_for_valid_br() {
        let instructions = vec![
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Br { offset: 0 },
        ];
        let function = make_function(instructions);
        let mut analyzer = BranchTargetAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_for_valid_brfalse() {
        let instructions = vec![
            Instruction::LdcBool {
                dest: Slot(0),
                value: false,
            },
            Instruction::LdcUnit { dest: Slot(1) },
            Instruction::BrFalse {
                var: Slot(0),
                offset: 3,
            },
            Instruction::Ret { var: Slot(1) },
        ];
        let function = make_function(instructions);
        let mut analyzer = BranchTargetAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_on_negative_br_offset() {
        let instructions = vec![
            Instruction::Br { offset: -1 },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(instructions);
        let mut analyzer = BranchTargetAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::InvalidBranchTarget {
                instruction_index: 0,
                target: -1,
            }
        );
    }

    #[test]
    fn warns_on_out_of_bounds_br_offset() {
        let instructions = vec![
            Instruction::Br { offset: 99 },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(instructions);
        let mut analyzer = BranchTargetAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::InvalidBranchTarget {
                instruction_index: 0,
                target: 99,
            }
        );
    }

    #[test]
    fn warns_on_each_invalid_switch_offset() {
        let instructions = vec![
            Instruction::LdcInt {
                dest: Slot(0),
                value: 0,
            },
            Instruction::Switch {
                var: Slot(0),
                offsets: vec![1, -1, 99],
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(instructions);
        let mut analyzer = BranchTargetAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = BranchTargetAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
