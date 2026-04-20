use super::Instruction;
use crate::runtime::{
    AgentMessageContent, Context, ExpressionParameter, ExpressionResult, ExpressionValue, Runtime,
};
use crate::types::ExecutableFunction;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use structured_agent_runtime::FunctionName;

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
        instructions: &[Instruction],
        context: Context,
    ) -> Result<(Context, ExpressionResult), String> {
        let mut state = VMState { pc: 0, context };

        loop {
            if state.pc >= instructions.len() {
                return Err("PC out of bounds".to_string());
            }

            let instruction = &instructions[state.pc];

            state = match instruction {
                Instruction::Nop => Self::advance_pc(state),
                Instruction::Drop { name } => self.execute_drop(state, name),
                Instruction::LdcStr { dest, value } => self.execute_ldc_str(state, dest, value),
                Instruction::LdcBool { dest, value } => self.execute_ldc_bool(state, dest, *value),
                Instruction::LdcInt { dest, value } => self.execute_ldc_int(state, dest, *value),
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
                Instruction::CallBytecode {
                    function_name,
                    params,
                    dest,
                } => {
                    self.execute_call(state, function_name, params, dest)
                        .await?
                }
                Instruction::CallExternal {
                    function_name,
                    params,
                    dest,
                } => {
                    self.execute_external_call(state, function_name, params, dest)
                        .await?
                }
                Instruction::LoadModule { name, dest } => {
                    self.execute_load_module(state, name, dest)
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
                Instruction::ListCreate { dest, elements } => {
                    self.execute_list_create(state, dest, elements)?
                }
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
                Instruction::StructNew {
                    dest,
                    struct_name: _,
                    fields,
                } => self.execute_struct_new(state, dest, fields)?,
                Instruction::StructGet { dest, src, field } => {
                    self.execute_struct_get(state, dest, src, field)?
                }
                Instruction::CallIndirect {
                    module_param,
                    fn_name,
                    params,
                    dest,
                } => {
                    self.execute_indirect_call(state, module_param, fn_name, params, dest)
                        .await?
                }
            };
        }
    }

    fn execute_ldc_str(&self, mut state: VMState, dest: &str, value: &str) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::string(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_bool(&self, mut state: VMState, dest: &str, value: bool) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::boolean(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_int(&self, mut state: VMState, dest: &str, value: i64) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::integer(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_unit(&self, mut state: VMState, dest: &str) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::unit()),
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
            ExpressionResult::new(ExpressionValue::unit()),
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

        let s = value.value.as_string().map_err(|_| {
            format!(
                "Expected string value for switch, got {}",
                value.value.type_name()
            )
        })?;
        let index = s
            .parse::<usize>()
            .map_err(|_| format!("Invalid switch index: {}", s))?;

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
        state: VMState,
        function_name: &FunctionName,
        params: &[String],
        dest: &str,
    ) -> Result<VMState, String> {
        let func = self
            .runtime
            .get_bytecode_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;
        self.invoke_function(state, func, &function_name.to_string(), params, dest)
            .await
    }

    async fn invoke_function(
        &self,
        mut state: VMState,
        func: Arc<dyn ExecutableFunction>,
        display_name: &str,
        params: &[String],
        dest: &str,
    ) -> Result<VMState, String> {
        let function_params = func.parameters();

        let mut args = Vec::new();
        for var_name in params.iter() {
            let value = Self::read_variable(&state, var_name)?;
            args.push(value.clone());
        }

        let leading_count = args.len().saturating_sub(function_params.len());

        let evaluated_parameters: Vec<ExpressionParameter> = args
            .iter()
            .enumerate()
            .skip(leading_count)
            .map(|(i, arg)| {
                ExpressionParameter::new(
                    function_params[i - leading_count].name.clone(),
                    arg.value.clone(),
                )
            })
            .collect();

        let mut child_context = state.context.create_child(true);

        for (i, var_name) in params.iter().enumerate().take(leading_count) {
            child_context.declare_variable(var_name.clone(), args[i].clone());
        }

        child_context.add_event(
            ExpressionValue::string(format!("## {}", display_name)),
            None,
            None,
        );

        let (returned_child_context, result) = func.execute(child_context, args).await?;

        state.context = returned_child_context.restore_parent()?;

        let result_with_metadata = ExpressionResult {
            name: Some(display_name.to_string()),
            params: Some(evaluated_parameters),
            value: result.value.clone(),
        };

        Self::write_variable(&mut state, dest, result_with_metadata);
        Ok(Self::advance_pc(state))
    }

    async fn execute_external_call(
        &self,
        state: VMState,
        function_name: &FunctionName,
        params: &[String],
        dest: &str,
    ) -> Result<VMState, String> {
        static CALL_COUNTER: AtomicU64 = AtomicU64::new(0);
        let call_id = CALL_COUNTER.fetch_add(1, Ordering::Relaxed).to_string();
        let name_str = function_name.to_string();
        let lookup_name = function_name.name().to_string();

        let resolved_params: HashMap<String, ExpressionValue> = params
            .iter()
            .filter_map(|name| {
                Self::read_variable(&state, name)
                    .ok()
                    .map(|r| (name.clone(), r.value))
            })
            .collect();

        state
            .context
            .agent_handle()
            .publish(AgentMessageContent::ToolCallStarted {
                tool_name: name_str.clone(),
                call_id: call_id.clone(),
                params: resolved_params,
            });

        let func = self
            .runtime
            .get_native_function(&lookup_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let state = self
            .invoke_function(state, func, &lookup_name, params, dest)
            .await?;

        let result = Self::read_variable(&state, dest)?;

        state
            .context
            .agent_handle()
            .publish(AgentMessageContent::ToolCallFinished {
                tool_name: name_str,
                call_id,
                result: result.value.clone(),
            });

        Ok(state)
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
        function_name: &FunctionName,
        dest: &str,
    ) -> Result<VMState, String> {
        let func = self
            .runtime
            .get_bytecode_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let name_str = function_name.to_string();
        let metadata =
            ExpressionValue::metadata(&name_str, func.documentation().map(|s| s.to_string()));

        Self::write_variable(&mut state, dest, ExpressionResult::new(metadata));
        Ok(Self::advance_pc(state))
    }

    fn execute_load_module(
        &self,
        mut state: VMState,
        name: &structured_agent_runtime::ModuleName,
        dest: &str,
    ) -> VMState {
        Self::write_variable(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::module(name.clone())),
        );
        Self::advance_pc(state)
    }

    fn execute_list_create(
        &self,
        mut state: VMState,
        dest: &str,
        element_vars: &[String],
    ) -> Result<VMState, String> {
        let elements: Vec<ExpressionValue> = element_vars
            .iter()
            .map(|var| Ok(Self::read_variable(&state, var)?.value))
            .collect::<Result<_, String>>()?;

        let list_value = ExpressionValue::from_elements(elements)?;
        Self::write_variable(&mut state, dest, ExpressionResult::new(list_value));
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_placeholder(
        &self,
        mut state: VMState,
        dest: &str,
        param_name: &str,
        param_type: &crate::types::Type,
    ) -> Result<VMState, String> {
        let value = state
            .context
            .runtime()
            .engine()
            .fill_parameter(&state.context, param_name, param_type)
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
            if value.value.type_name() != "Metadata" {
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

        let result = ExpressionResult::new(ExpressionValue::string(selected_index.to_string()));

        Self::write_variable(&mut state, dest, result);
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_generate(
        &self,
        mut state: VMState,
        dest: &str,
        return_type: &crate::types::Type,
    ) -> Result<VMState, String> {
        let value = state
            .context
            .runtime()
            .engine()
            .typed(&state.context, return_type)
            .await?;

        state
            .context
            .agent_handle()
            .publish(AgentMessageContent::String(value.format_for_llm()));

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

        let b = value.value.as_boolean().map_err(|_| {
            format!(
                "Expected boolean for branch, got {}",
                value.value.type_name()
            )
        })?;

        if b == expected {
            Ok(Self::branch(state, offset as usize))
        } else {
            Ok(Self::advance_pc(state))
        }
    }

    async fn execute_indirect_call(
        &self,
        state: VMState,
        module_param: &str,
        fn_name: &str,
        params: &[String],
        dest: &str,
    ) -> Result<VMState, String> {
        let module_val = Self::read_variable(&state, module_param)?;
        let module_name = module_val
            .value
            .as_module()
            .map_err(|e| format!("CallIndirect: {}", e))?
            .clone();
        let function_name = structured_agent_runtime::FunctionName::new(module_name, fn_name);
        let func = self
            .runtime
            .get_bytecode_function(&function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;
        self.invoke_function(state, func, &function_name.to_string(), params, dest)
            .await
    }

    fn execute_struct_new(
        &self,
        mut state: VMState,
        dest: &str,
        fields: &[(String, String)],
    ) -> Result<VMState, String> {
        let field_values: Vec<(&str, crate::runtime::ExpressionValue)> = fields
            .iter()
            .map(|(name, src)| {
                let val = Self::read_variable(&state, src)?;
                Ok((name.as_str(), val.value.clone()))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let struct_value = crate::runtime::ExpressionValue::struct_value(field_values);
        Self::write_variable(
            &mut state,
            dest,
            crate::runtime::ExpressionResult::new(struct_value),
        );
        Ok(Self::advance_pc(state))
    }

    fn execute_struct_get(
        &self,
        mut state: VMState,
        dest: &str,
        src: &str,
        field: &str,
    ) -> Result<VMState, String> {
        let src_val = Self::read_variable(&state, src)?;
        let field_value = src_val.value.get_struct_field(field)?;
        Self::write_variable(
            &mut state,
            dest,
            crate::runtime::ExpressionResult::new(field_value),
        );
        Ok(Self::advance_pc(state))
    }
}
