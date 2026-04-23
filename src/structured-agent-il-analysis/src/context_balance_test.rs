#[cfg(test)]
mod tests {
    use super::super::test_helpers::make_function;
    use crate::{ContextBalanceAnalyzer, IlAnalyzer, IlWarning};
    use structured_agent_il::{Instruction, Slot};

    #[test]
    fn no_warning_for_balanced_ctx() {
        let instructions = vec![
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::CtxRestore,
            Instruction::LdcUnit { dest: Slot(1) },
            Instruction::Ret { var: Slot(1) },
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
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Ret { var: Slot(0) },
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
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Ret { var: Slot(0) },
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
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Ret { var: Slot(0) },
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
            Instruction::Ret { var: Slot(0) },
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
