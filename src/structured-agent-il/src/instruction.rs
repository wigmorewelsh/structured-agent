use std::fmt;
use structured_agent_runtime::{DefinitionPath, Type};

use crate::slot::Slot;

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Nop,

    LdcStr {
        dest: Slot,
        value: String,
    },
    LdcBool {
        dest: Slot,
        value: bool,
    },
    LdcInt {
        dest: Slot,
        value: i64,
    },
    LdcUnit {
        dest: Slot,
    },

    Mov {
        dest: Slot,
        src: Slot,
    },

    Br {
        offset: i32,
    },
    BrFalse {
        var: Slot,
        offset: i32,
    },
    BrTrue {
        var: Slot,
        offset: i32,
    },
    Switch {
        var: Slot,
        offsets: Vec<i32>,
    },
    Ret {
        var: Slot,
    },
    Yield,

    CallBytecode {
        function_name: DefinitionPath,
        module_param_names: Vec<String>,
        params: Vec<Slot>,
        dest: Slot,
    },
    CallExternal {
        function_name: DefinitionPath,
        module_param_names: Vec<String>,
        params: Vec<Slot>,
        dest: Slot,
    },
    LoadModule {
        name: DefinitionPath,
        dest: Slot,
    },
    CallIndirect {
        module_param: String,
        fn_name: String,
        params: Vec<Slot>,
        dest: Slot,
    },

    CtxEvent {
        var: Slot,
    },
    CtxChild {
        is_scope_boundary: bool,
    },
    CtxRestore,

    MetaFunction {
        function_name: DefinitionPath,
        dest: Slot,
    },

    ListCreate {
        dest: Slot,
        elements: Vec<Slot>,
    },

    LlmPlaceholder {
        dest: Slot,
        param_name: String,
        param_type: Type,
    },
    LlmSelect {
        metadata_vars: Vec<Slot>,
        dest: Slot,
    },
    LlmGenerate {
        dest: Slot,
        return_type: Type,
    },

    StructNew {
        dest: Slot,
        struct_name: String,
        fields: Vec<(String, Slot)>,
    },
    StructGet {
        dest: Slot,
        src: Slot,
        field: String,
    },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::Nop => write!(f, "nop"),

            Instruction::LdcStr { dest, value } => {
                write!(f, "ldc.str {}, \"{}\"", dest, value.escape_default())
            }
            Instruction::LdcBool { dest, value } => write!(f, "ldc.bool {}, {}", dest, value),
            Instruction::LdcInt { dest, value } => write!(f, "ldc.int {}, {}", dest, value),
            Instruction::LdcUnit { dest } => write!(f, "ldc.unit {}", dest),

            Instruction::Mov { dest, src } => write!(f, "mov {}, {}", dest, src),

            Instruction::Br { offset } => write!(f, "br {}", offset),
            Instruction::BrFalse { var, offset } => write!(f, "brfalse {}, {}", var, offset),
            Instruction::BrTrue { var, offset } => write!(f, "brtrue {}, {}", var, offset),
            Instruction::Switch { var, offsets } => {
                write!(f, "switch {}, [", var)?;
                for (i, offset) in offsets.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", offset)?;
                }
                write!(f, "]")
            }
            Instruction::Ret { var } => write!(f, "ret {}", var),
            Instruction::Yield => write!(f, "yield"),

            Instruction::CallBytecode {
                function_name,
                params,
                dest,
                ..
            } => {
                write!(f, "call.bytecode {}, [", function_name)?;
                for (i, var) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", var)?;
                }
                write!(f, "], {}", dest)
            }
            Instruction::CallExternal {
                function_name,
                params,
                dest,
                ..
            } => {
                write!(f, "call.external {}, [", function_name)?;
                for (i, var) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", var)?;
                }
                write!(f, "], {}", dest)
            }

            Instruction::LoadModule { name, dest } => {
                write!(f, "load.module {}, {}", name, dest)
            }
            Instruction::CallIndirect {
                module_param,
                fn_name,
                params,
                dest,
            } => {
                write!(f, "call.indirect {}.{}, [", module_param, fn_name)?;
                for (i, var) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", var)?;
                }
                write!(f, "], {}", dest)
            }

            Instruction::CtxEvent { var } => write!(f, "ctx.event {}", var),
            Instruction::CtxChild { is_scope_boundary } => {
                write!(f, "ctx.child {}", is_scope_boundary)
            }
            Instruction::CtxRestore => write!(f, "ctx.restore"),

            Instruction::MetaFunction {
                function_name,
                dest,
            } => {
                write!(f, "meta.function {}, {}", function_name, dest)
            }

            Instruction::ListCreate { dest, elements } => {
                write!(f, "list.create {}, [", dest)?;
                for (i, var) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", var)?;
                }
                write!(f, "]")
            }

            Instruction::LlmPlaceholder {
                dest,
                param_name,
                param_type,
            } => {
                write!(
                    f,
                    "llm.placeholder {}, {}, {}",
                    dest, param_name, param_type
                )
            }
            Instruction::LlmSelect {
                metadata_vars,
                dest,
            } => {
                write!(f, "llm.select [")?;
                for (i, var) in metadata_vars.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", var)?;
                }
                write!(f, "], {}", dest)
            }
            Instruction::LlmGenerate { dest, return_type } => {
                write!(f, "llm.generate {}, {}", dest, return_type)
            }

            Instruction::StructNew {
                dest,
                struct_name,
                fields,
            } => {
                write!(f, "struct.new {}, {}, {{", dest, struct_name)?;
                for (i, (name, src)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", name, src)?;
                }
                write!(f, "}}")
            }
            Instruction::StructGet { dest, src, field } => {
                write!(f, "struct.get {}, {}, {}", dest, src, field)
            }
        }
    }
}
