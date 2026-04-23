#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{IlAnalyzer, IlWarning, VariableAllocationAnalyzer};
    use structured_agent_il::{BytecodeRef, Instruction, Slot, SlotTable};
    use structured_agent_runtime::{Parameter, Type};

    fn make_function(parameters: Vec<Parameter>, instructions: Vec<Instruction>) -> BytecodeRef {
        BytecodeRef {
            instructions,
            labels: HashMap::new(),
            parameters,
            return_type: Type::unit(),
            documentation: None,
            slot_table: SlotTable::new(),
        }
    }

    #[test]
    fn no_warning_when_variable_declared_before_use() {
        let instructions = vec![
            Instruction::LdcStr {
                dest: Slot(1),
                value: "hello".to_string(),
            },
            Instruction::Ret { var: Slot(1) },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_variable_used_before_decl() {
        let instructions = vec![Instruction::Ret { var: Slot(0) }];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::VariableUsedBeforeAllocation {
                name: "s0".to_string(),
                instruction_index: 0,
            }
        );
    }

    #[test]
    fn no_warning_when_parameter_used_without_decl() {
        let instructions = vec![
            Instruction::Mov {
                dest: Slot(0),
                src: Slot(1),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(
            vec![Parameter::new("x".to_string(), Type::unit())],
            instructions,
        );
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert!(warnings.is_empty());
    }

    #[test]
    fn warns_when_mov_src_not_declared() {
        let instructions = vec![
            Instruction::Mov {
                dest: Slot(0),
                src: Slot(2),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let function = make_function(vec![], instructions);
        let mut analyzer = VariableAllocationAnalyzer::new();
        let warnings = analyzer.analyze_function(&function);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            IlWarning::VariableUsedBeforeAllocation {
                name: "s2".to_string(),
                instruction_index: 0,
            }
        );
    }

    #[test]
    fn detects_multiple_undeclared_variables() {
        let instructions = vec![
            Instruction::BrFalse {
                var: Slot(1),
                offset: 2,
            },
            Instruction::Ret { var: Slot(2) },
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
