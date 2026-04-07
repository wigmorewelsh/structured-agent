#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{BytecodeRef, Instruction};
    use crate::il_analysis::{BranchTargetAnalyzer, IlAnalyzer, IlWarning};
    use crate::types::Type;

    fn make_function(instructions: Vec<Instruction>) -> BytecodeRef {
        BytecodeRef {
            instructions,
            labels: HashMap::new(),
            parameters: vec![],
            return_type: Type::Unit,
            documentation: None,
        }
    }

    #[test]
    fn no_warning_for_valid_br() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
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
            Instruction::Decl {
                name: "cond".to_string(),
            },
            Instruction::LdcBool {
                dest: "cond".to_string(),
                value: false,
            },
            Instruction::Decl {
                name: "$ret".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$ret".to_string(),
            },
            Instruction::BrFalse {
                var: "cond".to_string(),
                offset: 3,
            },
            Instruction::Ret {
                var: "$ret".to_string(),
            },
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
            Instruction::Ret {
                var: "$ret".to_string(),
            },
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
            Instruction::Ret {
                var: "$ret".to_string(),
            },
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
            Instruction::Decl {
                name: "idx".to_string(),
            },
            Instruction::LdcInt {
                dest: "idx".to_string(),
                value: 0,
            },
            Instruction::Switch {
                var: "idx".to_string(),
                offsets: vec![1, -1, 99],
            },
            Instruction::Ret {
                var: "idx".to_string(),
            },
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
