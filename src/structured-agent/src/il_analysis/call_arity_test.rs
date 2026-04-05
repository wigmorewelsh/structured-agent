#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{CompiledFunction, Instruction};
    use crate::il_analysis::{CallArityAnalyzer, IlAnalyzer, IlWarning};
    use crate::types::Type;
    use structured_agent_runtime::FunctionName;

    fn make_function(instructions: Vec<Instruction>) -> CompiledFunction {
        CompiledFunction {
            name: FunctionName::plain("", "test"),
            module_name: None,
            parameters: vec![],
            return_type: Type::Unit,
            instructions,
            labels: HashMap::new(),
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
                function_name: FunctionName::plain("", "greet"),
                params: vec!["$arg0".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("greet", 1)]));
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
                function_name: FunctionName::plain("", "get_value"),
                params: vec![],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("get_value", 0)]));
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
                function_name: FunctionName::plain("", "add"),
                params: vec!["$a".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("add", 2)]));
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::CallArityMismatch {
                function_name: "add".to_string(),
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
                function_name: FunctionName::plain("", "negate"),
                params: vec!["$a".to_string(), "$b".to_string()],
                dest: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("negate", 1)]));
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::CallArityMismatch {
                function_name: "negate".to_string(),
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
                function_name: FunctionName::plain("", "external_tool"),
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
                function_name: FunctionName::plain("", "foo"),
                params: vec![],
                dest: "$a".to_string(),
            },
            Instruction::CallBytecode {
                function_name: FunctionName::plain("", "bar"),
                params: vec!["$a".to_string(), "$b".to_string()],
                dest: "$b".to_string(),
            },
            Instruction::Ret {
                var: "$b".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("foo", 2), ("bar", 0)]));
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
