#[cfg(test)]
mod tests {
    use crate::{instruction_reads, instruction_writes};
    use structured_agent_il::Instruction;
    use structured_agent_il::slot::Slot;
    use structured_agent_runtime::{ExpressionValue, NativeFnPtr};

    fn make_ptr() -> NativeFnPtr {
        NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::unit()) }))
    }

    #[test]
    fn instruction_reads_call_native_returns_params() {
        let instr = Instruction::CallNative {
            f: make_ptr(),
            params: vec![Slot(1), Slot(2)],
            dest: Slot(0),
        };
        assert_eq!(instruction_reads(&instr), vec![Slot(1), Slot(2)]);
    }

    #[test]
    fn instruction_reads_str_concat_returns_parts() {
        let instr = Instruction::StrConcat {
            dest: Slot(0),
            parts: vec![Slot(1), Slot(2)],
        };
        assert_eq!(instruction_reads(&instr), vec![Slot(1), Slot(2)]);
    }

    #[test]
    fn instruction_writes_str_concat_returns_dest() {
        let instr = Instruction::StrConcat {
            dest: Slot(5),
            parts: vec![Slot(1)],
        };
        assert_eq!(instruction_writes(&instr), Some(Slot(5)));
    }

    #[test]
    fn instruction_writes_call_native_returns_dest() {
        let instr = Instruction::CallNative {
            f: make_ptr(),
            params: vec![Slot(1)],
            dest: Slot(3),
        };
        assert_eq!(instruction_writes(&instr), Some(Slot(3)));
    }
}
