#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{CallArityAnalyzer, IlAnalyzer, IlWarning};
    use nonempty::NonEmpty;
    use structured_agent_il::{Instruction, Slot};
    use structured_agent_runtime::DefinitionPath;

    use super::super::test_helpers::make_function;

    fn arities(pairs: &[(&str, usize)]) -> HashMap<String, usize> {
        pairs
            .iter()
            .map(|(name, arity)| (name.to_string(), *arity))
            .collect()
    }

    #[test]
    fn no_warning_when_arity_matches() {
        let instructions = vec![
            Instruction::LdcStr {
                dest: Slot(0),
                value: "hello".to_string(),
            },
            Instruction::CallBytecode {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "greet",
                ),
                module_param_names: vec![],
                params: vec![Slot(0)],
                dest: Slot(1),
            },
            Instruction::Ret { var: Slot(1) },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::greet", 1)]));
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_for_zero_arity_call() {
        let instructions = vec![
            Instruction::CallBytecode {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "get_value",
                ),
                module_param_names: vec![],
                params: vec![],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(arities(&[("test::get_value", 0)]));
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_too_few_args_provided() {
        let instructions = vec![
            Instruction::CallBytecode {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "add",
                ),
                module_param_names: vec![],
                params: vec![Slot(0)],
                dest: Slot(1),
            },
            Instruction::Ret { var: Slot(1) },
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
                instruction_index: 0,
            }
        );
    }

    #[test]
    fn warns_when_too_many_args_provided() {
        let instructions = vec![
            Instruction::CallBytecode {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "negate",
                ),
                module_param_names: vec![],
                params: vec![Slot(0), Slot(1)],
                dest: Slot(2),
            },
            Instruction::Ret { var: Slot(2) },
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
                instruction_index: 0,
            }
        );
    }

    #[test]
    fn no_warning_for_unknown_function() {
        let instructions = vec![
            Instruction::CallExternal {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "external_tool",
                ),
                module_param_names: vec![],
                params: vec![Slot(0), Slot(1), Slot(2)],
                dest: Slot(3),
            },
            Instruction::Ret { var: Slot(3) },
        ];
        let function = make_function(instructions);
        let mut analyzer = CallArityAnalyzer::new(HashMap::new());
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn detects_multiple_arity_mismatches() {
        let instructions = vec![
            Instruction::CallBytecode {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "foo",
                ),
                module_param_names: vec![],
                params: vec![],
                dest: Slot(0),
            },
            Instruction::CallBytecode {
                function_name: DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    "bar",
                ),
                module_param_names: vec![],
                params: vec![Slot(0), Slot(1)],
                dest: Slot(1),
            },
            Instruction::Ret { var: Slot(1) },
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
