use std::fmt;
use structured_agent_runtime::FunctionName;

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
    /// Load integer constant into variable
    LdcInt { dest: String, value: i64 },
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

    /// Call a bytecode function with parameters and store result in destination
    CallBytecode {
        function_name: FunctionName,
        params: Vec<String>,
        dest: String,
    },
    /// Call an external function with parameters and store result in destination
    CallExternal {
        function_name: FunctionName,
        params: Vec<String>,
        dest: String,
    },
    /// Load a module reference into a variable
    LoadModule { name: FunctionName, dest: String },

    /// Inject variable's value into context events (adds Event to context)
    CtxEvent { var: String },
    /// Create child context (true=function boundary, false=nested statement like loop/if/select)
    CtxChild { is_scope_boundary: bool },
    /// Return to parent context
    CtxRestore,

    /// Get metadata for a function
    MetaFunction { function_name: String, dest: String },

    /// Create list from element variables
    ListCreate { dest: String, elements: Vec<String> },

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

    /// Create a new struct value from named field variables
    StructNew {
        dest: String,
        struct_name: String,
        fields: Vec<(String, String)>,
    },
    /// Read a single field from a struct value into dest
    StructGet {
        dest: String,
        src: String,
        field: String,
    },
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
            Instruction::LdcInt { dest, value } => {
                write!(f, "ldc.int {}, {}", dest, value)
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

            Instruction::CallBytecode {
                function_name,
                params,
                dest,
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

            Instruction::LoadModule { name, dest } => {
                write!(f, "load.module {}, {}", name, dest)
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
