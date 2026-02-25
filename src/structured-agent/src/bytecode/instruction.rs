use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// No operation (used as jump target)
    Nop,

    /// Drop variable from context (cleanup temporary)
    Drop { name: String },

    /// Load string constant into variable
    LdcStr { dest: String, value: String },
    /// Load boolean constant into variable
    LdcBool { dest: String, value: bool },
    /// Load unit value into variable
    LdcUnit { dest: String },

    /// Copy variable value (full ExpressionResult)
    Mov { dest: String, src: String },
    /// Declare new variable in current context, allowing outer scope declaration before inner scope assignment
    Decl { name: String },

    /// Unconditional jump
    Br { offset: i32 },
    /// Jump if variable is false
    BrFalse { var: String, offset: i32 },
    /// Jump if variable is true
    BrTrue { var: String, offset: i32 },
    /// Jump based on variable's integer value
    Switch { var: String, offsets: Vec<i32> },
    /// Return with variable's value, exit function
    Ret { var: String },
    /// Pause execution for durable execution checkpoint
    Yield,

    /// Call function with parameters and store result in destination
    Call {
        function_name: String,
        params: Vec<String>,
        dest: String,
    },

    /// Inject variable's value into context events (adds Event to context)
    CtxEvent { var: String },
    /// Create child context (true=function boundary, false=nested statement like loop/if/select)
    CtxChild { is_scope_boundary: bool },
    /// Return to parent context
    CtxRestore,

    /// Get metadata for a function
    MetaFunction { function_name: String, dest: String },

    /// Create new list builder
    ListNew { dest: String, element_type: String },
    /// Append element to list builder
    ListAdd { dest: String, src: String },
    /// Finalize list builder into ListArray
    ListFinish { dest: String },

    /// Await LLM to fill placeholder, store in dest
    LlmPlaceholder {
        dest: String,
        param_name: String,
        param_type: String,
    },
    /// Await LLM clause choice, store selected index in dest
    LlmSelect {
        metadata_vars: Vec<String>,
        dest: String,
    },
    /// Await LLM generation with context, store result in dest
    LlmGenerate { dest: String, return_type: String },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::Nop => write!(f, "nop"),

            Instruction::Drop { name } => write!(f, "drop {}", name),

            Instruction::LdcStr { dest, value } => {
                write!(f, "ldc.str {}, \"{}\"", dest, value.escape_default())
            }
            Instruction::LdcBool { dest, value } => {
                write!(f, "ldc.bool {}, {}", dest, value)
            }
            Instruction::LdcUnit { dest } => {
                write!(f, "ldc.unit {}", dest)
            }

            Instruction::Mov { dest, src } => {
                write!(f, "mov {}, {}", dest, src)
            }
            Instruction::Decl { name } => {
                write!(f, "decl {}", name)
            }

            Instruction::Br { offset } => {
                write!(f, "br {}", offset)
            }
            Instruction::BrFalse { var, offset } => {
                write!(f, "brfalse {}, {}", var, offset)
            }
            Instruction::BrTrue { var, offset } => {
                write!(f, "brtrue {}, {}", var, offset)
            }
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
            Instruction::Ret { var } => {
                write!(f, "ret {}", var)
            }
            Instruction::Yield => {
                write!(f, "yield")
            }

            Instruction::Call {
                function_name,
                params,
                dest,
            } => {
                write!(f, "call {}, [", function_name)?;
                for (i, var) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", var)?;
                }
                write!(f, "], {}", dest)
            }

            Instruction::CtxEvent { var } => {
                write!(f, "ctx.event {}", var)
            }
            Instruction::CtxChild { is_scope_boundary } => {
                write!(f, "ctx.child {}", is_scope_boundary)
            }
            Instruction::CtxRestore => {
                write!(f, "ctx.restore")
            }

            Instruction::MetaFunction {
                function_name,
                dest,
            } => {
                write!(f, "meta.function {}, {}", function_name, dest)
            }

            Instruction::ListNew { dest, element_type } => {
                write!(f, "list.new {}, {}", dest, element_type)
            }
            Instruction::ListAdd { dest, src } => {
                write!(f, "list.add {}, {}", dest, src)
            }
            Instruction::ListFinish { dest } => {
                write!(f, "list.finish {}", dest)
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
        }
    }
}
