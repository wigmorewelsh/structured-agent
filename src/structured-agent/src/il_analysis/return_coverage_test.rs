#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::bytecode::{CompiledFunction, Instruction};
    use crate::il_analysis::{IlAnalyzer, IlWarning, ReturnCoverageAnalyzer};
    use crate::types::Type;

    fn make_function(instructions: Vec<Instruction>) -> CompiledFunction {
        CompiledFunction {
            name: "test".to_string(),
            module_name: None,
            parameters: vec![],
            return_type: Type::Unit,
            instructions,
            labels: HashMap::new(),
            documentation: None,
        }
    }

    #[test]
    fn no_warning_when_function_ends_with_ret() {
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
        let mut analyzer = ReturnCoverageAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warning_when_ret_is_not_last_instruction() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
            Instruction::Ret {
                var: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = ReturnCoverageAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_no_ret_in_non_empty_function() {
        let instructions = vec![
            Instruction::Decl {
                name: "$tmp0".to_string(),
            },
            Instruction::LdcUnit {
                dest: "$tmp0".to_string(),
            },
        ];
        let function = make_function(instructions);
        let mut analyzer = ReturnCoverageAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0], IlWarning::NoReturnPath);
    }

    #[test]
    fn no_warning_for_empty_function() {
        let function = make_function(vec![]);
        let mut analyzer = ReturnCoverageAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }
}
