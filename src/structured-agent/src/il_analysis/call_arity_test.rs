#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{BytecodeRef, Instruction};
    use crate::il_analysis::{CallArityAnalyzer, IlAnalyzer, IlWarning};
    use crate::types::Type;
    use nonempty::NonEmpty;
    use structured_agent_runtime::{FunctionName, FunctionNameKind, ModuleName};

    fn make_function(instructions: Vec<Instruction>) -> BytecodeRef {
        BytecodeRef {
            instructions,
            labels: HashMap::new(),
            parameters: vec![],
            return_type: Type::Unit,
            documentation: None,
        }
    }

    fn arities(pairs: &[(&str, usize)]) -> HashMap<String, usize> {
        pairs
            .iter()
            .map(|(name, arity)| (name.to_string(), *arity))
            .collect()
    }

    #[test]
    fn no_warning_when_arity_matches() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::Decl {
                name: "$arg0".to_string(),
            },
            Instruction::LdcStr {
                dest: "$arg0".to_string(),
                value: "hello".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName {
                    name: "greet".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec!["$arg0".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::greet", 1)]));
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_for_zero_arity_call() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName {
                    name: "get_value".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec![],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::get_value", 0)]));
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_too_few_args_provided() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName {
                    name: "add".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec!["$a".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::add", 2)]));
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::CallArityMismatch {
                function_name: "test::add".to_string(),
                expected: 2,
                got: 1,
                instruction_index: 1,
            }
        );
    }

    #[test]
    fn warns_when_too_many_args_provided() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName {
                    name: "negate".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec!["$a".to_string(), "$b".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::negate", 1)]));
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::CallArityMismatch {
                function_name: "test::negate".to_string(),
                expected: 1,
                got: 2,
                instruction_index: 1,
            }
        );
    }

    #[test]
    fn no_warning_for_unknown_function() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::CallExternal {
                function_name: FunctionName {
                    name: "external_tool".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec!["$a".to_string(), "$b".to_string(), "$c".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(HashMap::new());
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn detects_multiple_arity_mismatches() {
        let instructions = vec![
            Instruction::Decl {
                name: "$a".to_string(),
            },
            Instruction::Decl {
                name: "$b".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName {
                    name: "foo".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec![],
                dest: "$a".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName {
                    name: "bar".to_string(),
                    module: ModuleName::new(NonEmpty::new("test".to_string())),
                    kind: FunctionNameKind::Function,
                },
                params: vec!["$a".to_string(), "$b".to_string()],
                dest: "$b".to_string(),
            },
            Instruction::Ret {
                var: "$b".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::foo", 2), ("test::bar", 0)]));
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = CallArityAnalyzer::new(HashMap::new());
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
