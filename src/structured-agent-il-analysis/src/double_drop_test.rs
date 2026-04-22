#[cfg(test)]
mod tests {
    use crate::{DoubleDropAnalyzer, IlAnalyzer, IlWarning};
    use structured_agent_il::Instruction;

    use super::super::test_helpers::make_function;

    #[test]
    fn no_warning_when_each_variable_dropped_once() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcStr {
                dest: "$tmp0".to_string(),
                value: "hello".to_string(),
            },
            Instruction::Drop {
                name: "$tmp0".to_string(),
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
        let mut analyzer = DoubleDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_same_variable_dropped_twice() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
            Instruction::Drop {
                name: "$tmp0".to_string(),
            },
            Instruction::Drop {
                name: "$tmp0".to_string(),
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
        let mut analyzer = DoubleDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::DoubleDrop {
                name: "$tmp0".to_string(),
                instruction_index: 3,
            }
        );
    }

    #[test]
    fn detects_multiple_double_drops() {
        let instructions = vec![
            Instruction::Decl {
                name: "a".to_string(),
            },
            Instruction::Decl {
                name: "b".to_string(),
            },
            Instruction::Drop {
                name: "a".to_string(),
            },
            Instruction::Drop {
                name: "b".to_string(),
            },
            Instruction::Drop {
                name: "a".to_string(),
            },
            Instruction::Drop {
                name: "b".to_string(),
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
        let mut analyzer = DoubleDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = DoubleDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
