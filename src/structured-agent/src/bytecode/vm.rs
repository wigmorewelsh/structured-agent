use super::{CompiledFunction, Instruction};
use crate::runtime::{Context, ExpressionParameter, ExpressionResult, ExpressionValue, Runtime};
use std::sync::Arc;
use tracing::info;

pub struct VMState {
    pc: usize,
    context: Context,
}

pub struct VM {
    runtime: Arc<Runtime>,
}

impl VM {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }

    pub async fn execute(
        &self,
        function: &CompiledFunction,
        context: Context,
    ) -> Result<(Context, ExpressionResult), String> {
        let mut state = VMState { pc: 0, context };

        loop {
            if state.pc >= function.instructions.len() {
                return Err("PC out of bounds".to_string());
            }

            let instruction = &function.instructions[state.pc];

            state = match instruction {
                Instruction::Nop => Self::advance_pc(state),
                Instruction::Drop { name } => self.execute_drop(state, name),
                Instruction::LdcStr { dest, value } => self.execute_ldc_str(state, dest, value),
                Instruction::LdcBool { dest, value } => self.execute_ldc_bool(state, dest, *value),
                Instruction::LdcUnit { dest } => self.execute_ldc_unit(state, dest),
                Instruction::Mov { dest, src } => self.execute_mov(state, dest, src)?,
                Instruction::Decl { name } => self.execute_decl(state, name),
                Instruction::Br { offset } => Self::branch(state, *offset as usize),
                Instruction::BrFalse { var, offset } => {
                    Self::branch_if_bool(state, var, *offset, false)?
                }
                Instruction::BrTrue { var, offset } => {
                    Self::branch_if_bool(state, var, *offset, true)?
                }
                Instruction::Switch { var, offsets } => self.execute_switch(state, var, offsets)?,
                Instruction::Ret { var } => {
                    let (state, result) = self.execute_ret(state, var)?;
                    return Ok((state.context, result));
                }
                Instruction::Yield => return Err("Yield not yet implemented".to_string()),
                Instruction::Call {
                    function_name,
                    params,
                    dest,
                } => {
                    self.execute_call(state, function_name, params, dest)
                        .await?
                }
                Instruction::CtxEvent { var } => self.execute_ctx_event(state, var)?,
                Instruction::CtxChild { is_scope_boundary } => {
                    self.execute_ctx_child(state, *is_scope_boundary)
                }
                Instruction::CtxRestore => self.execute_ctx_restore(state)?,
                Instruction::MetaFunction {
                    function_name,
                    dest,
                } => self.execute_meta_function(state, function_name, dest)?,
                Instruction::ListNew {
                    dest,
                    element_type: _,
                } => self.execute_list_new(state, dest),
                Instruction::ListAdd { dest: _, src: _ } => Self::advance_pc(state),
                Instruction::ListFinish { dest: _ } => Self::advance_pc(state),
                Instruction::LlmPlaceholder {
                    dest,
                    param_name,
                    param_type,
                } => {
                    self.execute_llm_placeholder(state, dest, param_name, param_type)
                        .await?
                }
                Instruction::LlmSelect {
                    metadata_vars,
                    dest,
                } => self.execute_llm_select(state, metadata_vars, dest).await?,
                Instruction::LlmGenerate { dest, return_type } => {
                    self.execute_llm_generate(state, dest, return_type).await?
                }
            };
        }
    }

    fn execute_ldc_str(&self, mut state: VMState, dest: &str, value: &str) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::String(value.to_string())),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_bool(&self, mut state: VMState, dest: &str, value: bool) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::Boolean(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_unit(&self, mut state: VMState, dest: &str) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::Unit),
        );
        Self::advance_pc(state)
    }

    fn execute_mov(&self, mut state: VMState, dest: &str, src: &str) -> Result<VMState, String> {
        let value = Self::read_variable(&state, src)?;
        state.context.assign_variable(dest.to_string(), value)?;
        Ok(Self::advance_pc(state))
    }

    fn execute_decl(&self, mut state: VMState, name: &str) -> VMState {
        Self::write_variable(
            &mut state,
            name,
            ExpressionResult::new(ExpressionValue::Unit),
        );
        Self::advance_pc(state)
    }

    fn execute_drop(&self, mut state: VMState, name: &str) -> VMState {
        state.context.remove_variable(name);
        Self::advance_pc(state)
    }

    fn execute_switch(
        &self,
        state: VMState,
        var: &str,
        offsets: &[i32],
    ) -> Result<VMState, String> {
        let value = Self::read_variable(&state, var)?;

        let index = match &value.value {
            ExpressionValue::String(s) => s
                .parse::<usize>()
                .map_err(|_| format!("Invalid switch index: {}", s))?,
            _ => {
                return Err(format!(
                    "Expected string value for switch, got {:?}",
                    value.value
                ));
            }
        };

        if index < offsets.len() {
            Ok(Self::branch(state, offsets[index] as usize))
        } else {
            Err(format!("Switch index {} out of range", index))
        }
    }

    fn execute_ret(
        &self,
        mut state: VMState,
        var: &str,
    ) -> Result<(VMState, ExpressionResult), String> {
        let result = Self::read_variable(&state, var)?;
        state.context.set_return_value(result.clone());
        Ok((state, result))
    }

    async fn execute_call(
        &self,
        mut state: VMState,
        function_name: &str,
        params: &[String],
        dest: &str,
    ) -> Result<VMState, String> {
        let func = self
            .runtime
            .get_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let function_params = func.parameters();

        let mut args = Vec::new();
        for var_name in params.iter() {
            let value = Self::read_variable(&state, var_name)?;
            args.push(value.clone());
        }

        let evaluated_parameters: Vec<ExpressionParameter> = args
            .iter()
            .enumerate()
            .map(|(i, arg)| {
                ExpressionParameter::new(function_params[i].name.clone(), arg.value.clone())
            })
            .collect();

        let mut child_context = state.context.create_child(true);

        child_context.add_event(
            ExpressionValue::String(format!("## {}", function_name)),
            None,
            None,
        );

        let (returned_child_context, result) = func.execute(child_context, args).await?;

        state.context = returned_child_context.restore_parent()?;

        let result_with_metadata = ExpressionResult {
            name: Some(function_name.to_string()),
            params: Some(evaluated_parameters),
            value: result.value.clone(),
        };

        let result_display = match &result.value {
            ExpressionValue::String(s) => s.clone(),
            ExpressionValue::Boolean(b) => b.to_string(),
            ExpressionValue::Unit => "()".to_string(),
            _ => format!("{:?}", result.value),
        };

        info!(
            "<result function=\"{}\">\n{}\n</result>",
            function_name, result_display
        );

        Self::write_variable(&mut state, dest, result_with_metadata);
        Ok(Self::advance_pc(state))
    }

    fn execute_ctx_event(&self, mut state: VMState, var: &str) -> Result<VMState, String> {
        let expr_result = Self::read_variable(&state, var)?;

        state.context.add_event(
            expr_result.value.clone(),
            expr_result.name.clone(),
            expr_result.params.clone(),
        );
        Ok(Self::advance_pc(state))
    }

    fn execute_ctx_child(&self, state: VMState, is_scope_boundary: bool) -> VMState {
        let child_context = state.context.create_child(is_scope_boundary);
        let new_state = VMState {
            pc: state.pc,
            context: child_context,
        };
        Self::advance_pc(new_state)
    }

    fn execute_ctx_restore(&self, state: VMState) -> Result<VMState, String> {
        let parent_context = state.context.restore_parent()?;
        let new_state = VMState {
            pc: state.pc,
            context: parent_context,
        };
        Ok(Self::advance_pc(new_state))
    }

    fn execute_meta_function(
        &self,
        mut state: VMState,
        function_name: &str,
        dest: &str,
    ) -> Result<VMState, String> {
        let func = self
            .runtime
            .get_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let metadata = ExpressionValue::Metadata {
            name: function_name.to_string(),
            documentation: func.documentation().map(|s| s.to_string()),
        };

        Self::write_variable(&mut state, dest, ExpressionResult::new(metadata));
        Ok(Self::advance_pc(state))
    }

    fn execute_list_new(&self, mut state: VMState, dest: &str) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::Unit),
        );
        Self::advance_pc(state)
    }

    async fn execute_llm_placeholder(
        &self,
        mut state: VMState,
        dest: &str,
        param_name: &str,
        param_type: &str,
    ) -> Result<VMState, String> {
        let param_type_obj = parse_type(param_type)?;
        let value = state
            .context
            .runtime()
            .engine()
            .fill_parameter(&state.context, param_name, &param_type_obj)
            .await?;

        Self::write_variable(&mut state, dest, ExpressionResult::new(value));
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_select(
        &self,
        mut state: VMState,
        metadata_vars: &[String],
        dest: &str,
    ) -> Result<VMState, String> {
        let mut metadata_values = Vec::new();

        for var_name in metadata_vars {
            let value = Self::read_variable(&state, var_name)?;
            if !matches!(&value.value, ExpressionValue::Metadata { .. }) {
                return Err(format!(
                    "Expected Metadata value in variable {}, got {}",
                    var_name,
                    value.value.type_name()
                ));
            }
            metadata_values.push(value.value.clone());
        }

        let selected_index = state
            .context
            .runtime()
            .engine()
            .select(&state.context, &metadata_values)
            .await?;

        let result = ExpressionResult::new(ExpressionValue::String(selected_index.to_string()));

        Self::write_variable(&mut state, dest, result);
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_generate(
        &self,
        mut state: VMState,
        dest: &str,
        return_type: &str,
    ) -> Result<VMState, String> {
        let return_type_obj = parse_type(return_type)?;
        let value = state
            .context
            .runtime()
            .engine()
            .typed(&state.context, &return_type_obj)
            .await?;

        Self::write_variable(&mut state, dest, ExpressionResult::new(value));
        Ok(Self::advance_pc(state))
    }

    fn advance_pc(mut state: VMState) -> VMState {
        state.pc += 1;
        state
    }

    fn branch(mut state: VMState, offset: usize) -> VMState {
        state.pc = offset;
        state
    }

    fn read_variable(state: &VMState, name: &str) -> Result<ExpressionResult, String> {
        state
            .context
            .get_variable(name)
            .ok_or_else(|| format!("Variable not found: {}", name))
    }

    fn write_variable(state: &mut VMState, name: &str, value: ExpressionResult) {
        state.context.declare_variable(name.to_string(), value);
    }

    fn branch_if_bool(
        state: VMState,
        var: &str,
        offset: i32,
        expected: bool,
    ) -> Result<VMState, String> {
        let value = Self::read_variable(&state, var)?;

        match &value.value {
            ExpressionValue::Boolean(b) if *b == expected => {
                Ok(Self::branch(state, offset as usize))
            }
            ExpressionValue::Boolean(_) => Ok(Self::advance_pc(state)),
            _ => Err(format!(
                "Expected boolean for branch, got {:?}",
                value.value
            )),
        }
    }
}

fn parse_type(type_str: &str) -> Result<crate::types::Type, String> {
    match type_str {
        "String" => Ok(crate::types::Type::String),
        "Boolean" => Ok(crate::types::Type::Boolean),
        "Unit" | "()" => Ok(crate::types::Type::Unit),
        "Unknown" => Ok(crate::types::Type::String),
        s if s.starts_with("List<") && s.ends_with(">") => {
            let inner = &s[5..s.len() - 1];
            Ok(crate::types::Type::List(Box::new(parse_type(inner)?)))
        }
        s if s.starts_with("Option<") && s.ends_with(">") => {
            let inner = &s[7..s.len() - 1];
            Ok(crate::types::Type::Option(Box::new(parse_type(inner)?)))
        }
        _ => Err(format!("Unknown type: {}", type_str)),
    }
}
