#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{CompiledFunction, Instruction};
    use crate::il_analysis::{ContextBalanceAnalyzer, IlAnalyzer, IlWarning};
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

    #[test]
    fn no_warning_for_balanced_ctx() {
        let instructions = vec![
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
            Instruction::CtxRestore,
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
        let mut analyzer = ContextBalanceAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_for_nested_balanced_ctx() {
        let instructions = vec![
            Instruction::CtxChild {
                is_scope_boundary: true,
            },
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::CtxRestore,
            Instruction::CtxRestore,
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
        let mut analyzer = ContextBalanceAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_on_ctx_restore_without_child() {
        let instructions = vec![
            Instruction::CtxRestore,
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
        let mut analyzer = ContextBalanceAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::ContextUnderflow {
                instruction_index: 0,
            }
        );
    }

    #[test]
    fn warns_when_ctx_child_not_restored() {
        let instructions = vec![
            Instruction::CtxChild {
                is_scope_boundary: false,
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
        let mut analyzer = ContextBalanceAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], IlWarning::ContextNotRestored { depth: 1 });
    }

    #[test]
    fn warns_once_per_unrestored_depth() {
        let instructions = vec![
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Ret {
                var: "$ret".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = ContextBalanceAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], IlWarning::ContextNotRestored { depth: 2 });
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = ContextBalanceAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
