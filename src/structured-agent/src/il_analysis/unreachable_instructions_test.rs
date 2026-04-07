#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{BytecodeRef, Instruction};
    use crate::il_analysis::{IlAnalyzer, IlWarning, UnreachableInstructionAnalyzer};
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
    fn no_warning_for_normal_function() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_on_instruction_after_ret() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
            Instruction::Decl {
                name: "$tmp1".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::UnreachableInstruction {
                instruction_index: 3,
            }
        );
    }

    #[test]
    fn warns_on_instruction_after_unconditional_br() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
            Instruction::Br { offset: 0 },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::UnreachableInstruction {
                instruction_index: 3,
            }
        );
    }

    #[test]
    fn no_warning_when_instruction_after_br_is_a_jump_target() {
        let instructions = vec![
            Instruction::Decl {
                name: "cond".to_string(),
            },
            Instruction::LdcBool {
                dest: "cond".to_string(),
                value: false,
            },
            Instruction::BrFalse {
                var: "cond".to_string(),
                offset: 4,
            },
            Instruction::Br { offset: 4 },
            Instruction::Decl {
                name: "$ret".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$ret".to_string(),
            },
            Instruction::Ret {
                var: "$ret".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = UnreachableInstructionAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn conditional_branch_does_not_make_next_instruction_unreachable() {
        let instructions = vec![
            Instruction::Decl {
                name: "cond".to_string(),
            },
            Instruction::LdcBool {
                dest: "cond".to_string(),
                value: true,
            },
            Instruction::BrFalse {
                var: "cond".to_string(),
                offset: 4,
            },
            Instruction::Decl {
                name: "$ret".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$ret".to_string(),
            },
            Instruction::Ret {
                var: "$ret".to_string(),
            },
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
