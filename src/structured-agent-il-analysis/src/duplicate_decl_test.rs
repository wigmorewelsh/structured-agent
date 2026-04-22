#[cfg(test)]
mod tests {
    use crate::{DuplicateDeclAnalyzer, IlAnalyzer, IlWarning};
    use structured_agent_il::Instruction;

    use super::super::test_helpers::make_function;

    #[test]
    fn no_warning_when_each_variable_declared_once() {
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
        let function = make_function(instructions);
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_same_variable_declared_twice_in_same_scope() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::DuplicateDeclaration {
                name: "$tmp0".to_string(),
                instruction_index: 2,
            }
        );
    }

    #[test]
    fn detects_multiple_duplicate_declarations_in_same_scope() {
        let instructions = vec![
            Instruction::Decl {
                name: "x".to_string(),
            },
            Instruction::Decl {
                name: "y".to_string(),
            },
            Instruction::Decl {
                name: "x".to_string(),
            },
            Instruction::Decl {
                name: "y".to_string(),
            },
            Instruction::Ret {
                var: "x".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 2);
    }

    #[test]
    fn no_warning_for_same_name_in_sibling_child_scopes() {
        let instructions = vec![
            Instruction::Decl {
                name: "dest".to_string(),
            },
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Decl {
                name: "r".to_string(),
            },
            Instruction::Mov {
                dest: "dest".to_string(),
                src: "r".to_string(),
            },
            Instruction::CtxRestore,
            Instruction::Br { offset: 10 },
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Decl {
                name: "r".to_string(),
            },
            Instruction::Mov {
                dest: "dest".to_string(),
                src: "r".to_string(),
            },
            Instruction::CtxRestore,
            Instruction::Ret {
                var: "dest".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_for_shadowing_in_child_scope() {
        let instructions = vec![
            Instruction::Decl {
                name: "loop_once".to_string(),
            },
            Instruction::LdcBool {
                dest: "loop_once".to_string(),
                value: true,
            },
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Decl {
                name: "loop_once".to_string(),
            },
            Instruction::LdcBool {
                dest: "loop_once".to_string(),
                value: false,
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
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_for_duplicate_within_child_scope() {
        let instructions = vec![
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Decl {
                name: "x".to_string(),
            },
            Instruction::Decl {
                name: "x".to_string(),
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
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::DuplicateDeclaration {
                name: "x".to_string(),
                instruction_index: 2,
            }
        );
    }

    #[test]
    fn warns_for_dest_var_redeclared_at_root_scope() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::CtxChild {
                is_scope_boundary: false,
            },
            Instruction::Decl {
                name: "r".to_string(),
            },
            Instruction::Mov {
                dest: "$tmp0".to_string(),
                src: "r".to_string(),
            },
            Instruction::CtxRestore,
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::DuplicateDeclaration {
                name: "$tmp0".to_string(),
                instruction_index: 1,
            }
        );
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = DuplicateDeclAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
