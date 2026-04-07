#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{CompiledFunction, Instruction};
    use crate::il_analysis::{IlAnalyzer, IlWarning, VariableAllocationAnalyzer};
    use crate::types::{Parameter, Type};
    use structured_agent_runtime::{FunctionName, FunctionNameKind, ModuleName};

    fn make_function(
        parameters: Vec<Parameter>,
        instructions: Vec<Instruction>,
    ) -> CompiledFunction {
        CompiledFunction {
            name: FunctionName {
                name: "test".to_string(),
                module: ModuleName::from_str(""),
                kind: FunctionNameKind::Function,
            },
            module_name: None,
            parameters,
            return_type: Type::Unit,
            instructions,
            labels: HashMap::new(),
            documentation: None,
        }
    }

    #[test]
    fn no_warning_when_variable_declared_before_use() {
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
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_variable_used_before_decl() {
        let instructions = vec![Instruction::Ret {
            var: "$tmp0".to_string(),
        }];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::VariableUsedBeforeAllocation {
                name: "$tmp0".to_string(),
                instruction_index: 0,
            }
        );
    }

    #[test]
    fn no_warning_when_parameter_used_without_decl() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::Mov {
                dest: "$tmp0".to_string(),
                src: "x".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(
            vec![Parameter::new("x".to_string(), Type::Unit)],
            instructions,
        );
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_mov_src_not_declared() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::Mov {
                dest: "$tmp0".to_string(),
                src: "undeclared".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::VariableUsedBeforeAllocation {
                name: "undeclared".to_string(),
                instruction_index: 1,
            }
        );
    }

    #[test]
    fn detects_multiple_undeclared_variables() {
        let instructions = vec![
            Instruction::BrFalse {
                var: "cond".to_string(),
                offset: 2,
            },
            Instruction::Ret {
                var: "result".to_string(),
            },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![], vec![]);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
