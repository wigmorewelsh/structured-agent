#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{BytecodeRef, Instruction};
    use crate::il_analysis::{IlAnalyzer, IlWarning, VariableDropAnalyzer};
    use crate::types::{Parameter, Type};

    fn make_function(parameters: Vec<Parameter>, instructions: Vec<Instruction>) -> BytecodeRef {
        BytecodeRef {
            instructions,
            labels: HashMap::new(),
            parameters,
            return_type: Type::Unit,
            documentation: None,
        }
    }

    #[test]
    fn no_warning_when_variable_is_dropped() {
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
                name: "$tmp1".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp1".to_string(),
            },
            Instruction::Ret {
                var: "$tmp1".to_string(),
            },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_when_variable_is_returned() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcStr {
                dest: "$tmp0".to_string(),
                value: "hello".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_declared_variable_is_never_dropped_or_returned() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcStr {
                dest: "$tmp0".to_string(),
                value: "hello".to_string(),
            },
            Instruction::Decl {
                name: "$tmp1".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp1".to_string(),
            },
            Instruction::Ret {
                var: "$tmp1".to_string(),
            },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::VariableNotDropped {
                name: "$tmp0".to_string(),
            }
        );
    }

    #[test]
    fn warns_for_each_undropped_variable() {
        let instructions = vec![
            Instruction::Decl {
                name: "a".to_string(),
            },
            Instruction::Decl {
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
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![], vec![]);
        let mut analyzer = VariableDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_when_all_variables_dropped_and_one_returned() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcStr {
                dest: "$tmp0".to_string(),
                value: "event".to_string(),
            },
            Instruction::CtxEvent {
                var: "$tmp0".to_string(),
            },
            Instruction::Drop {
                name: "$tmp0".to_string(),
            },
            Instruction::Decl {
                name: "$tmp1".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp1".to_string(),
            },
            Instruction::Ret {
                var: "$tmp1".to_string(),
            },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableDropAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
