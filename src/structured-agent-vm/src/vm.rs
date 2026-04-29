use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use structured_agent_il::Instruction;
use structured_agent_il::slot::Slot;
use structured_agent_interpreter_runtime::{
    AgentMessageContent, Context, ExecutableFunction, ExpressionParameter, ExpressionResult,
    ExpressionValue, FillParameterEvent, RuntimeService, SelectEvent, TypedEvent,
};
use structured_agent_runtime::{DefinitionPath, NativeFnPtr};

pub struct VMState {
    pc: usize,
    context: Context,
    frame: Vec<Option<ExpressionResult>>,
}

pub struct VM {
    runtime: Arc<dyn RuntimeService>,
}

impl VM {
    pub fn new(runtime: Arc<dyn RuntimeService>) -> Self {
        Self { runtime }
    }

    pub async fn execute(
        &self,
        instructions: &[Instruction],
        context: Context,
        frame: Vec<Option<ExpressionResult>>,
    ) -> Result<(Context, ExpressionResult), String> {
        let mut state = VMState {
            pc: 0,
            context,
            frame,
        };

        loop {
            if state.pc >= instructions.len() {
                return Err("PC out of bounds".to_string());
            }

            let instruction = &instructions[state.pc];

            state = match instruction {
                Instruction::Nop => Self::advance_pc(state),
                Instruction::LdcStr { dest, value } => self.execute_ldc_str(state, *dest, value),
                Instruction::LdcBool { dest, value } => self.execute_ldc_bool(state, *dest, *value),
                Instruction::LdcInt { dest, value } => self.execute_ldc_int(state, *dest, *value),
                Instruction::LdcUnit { dest } => self.execute_ldc_unit(state, *dest),
                Instruction::Mov { dest, src } => self.execute_mov(state, *dest, *src)?,
                Instruction::Br { offset } => Self::branch(state, *offset as usize),
                Instruction::BrFalse { var, offset } => {
                    Self::branch_if_bool(state, *var, *offset, false)?
                }
                Instruction::BrTrue { var, offset } => {
                    Self::branch_if_bool(state, *var, *offset, true)?
                }
                Instruction::Switch { var, offsets } => {
                    self.execute_switch(state, *var, offsets)?
                }
                Instruction::Ret { var } => {
                    let (state, result) = self.execute_ret(state, *var)?;
                    return Ok((state.context, result));
                }
                Instruction::Yield => return Err("Yield not yet implemented".to_string()),
                Instruction::CallBytecode {
                    function_name,
                    params,
                    dest,
                } => {
                    self.execute_call(state, function_name, params, *dest)
                        .await?
                }
                Instruction::CallExternal {
                    function_name,
                    params,
                    dest,
                } => {
                    self.execute_external_call(state, function_name, params, *dest)
                        .await?
                }
                Instruction::LoadModule { name, params, dest } => {
                    self.execute_load_module(state, name, params, *dest)?
                }
                Instruction::CtxEvent { var } => self.execute_ctx_event(state, *var)?,
                Instruction::CtxChild => self.execute_ctx_child(state),
                Instruction::CtxRestore => self.execute_ctx_restore(state)?,
                Instruction::MetaFunction {
                    function_name,
                    dest,
                } => self.execute_meta_function(state, function_name, *dest)?,
                Instruction::ListCreate { dest, elements } => {
                    self.execute_list_create(state, *dest, elements)?
                }
                Instruction::LlmPlaceholder {
                    dest,
                    param_name,
                    param_type,
                } => {
                    self.execute_llm_placeholder(state, *dest, param_name, param_type)
                        .await?
                }
                Instruction::LlmSelect {
                    metadata_vars,
                    dest,
                } => self.execute_llm_select(state, metadata_vars, *dest).await?,
                Instruction::LlmGenerate { dest, return_type } => {
                    self.execute_llm_generate(state, *dest, return_type).await?
                }
                Instruction::StructNew {
                    dest,
                    struct_name: _,
                    fields,
                } => self.execute_struct_new(state, *dest, fields)?,
                Instruction::StructGet { dest, src, field } => {
                    self.execute_struct_get(state, *dest, *src, field)?
                }
                Instruction::CallIndirect {
                    module_param,
                    fn_name,
                    params,
                    dest,
                } => {
                    self.execute_indirect_call(state, *module_param, fn_name, params, *dest)
                        .await?
                }
                Instruction::CallNative { f, params, dest } => {
                    self.execute_native_call(state, f, params, *dest).await?
                }
            };
        }
    }

    fn execute_ldc_str(&self, mut state: VMState, dest: Slot, value: &str) -> VMState {
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::string(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_bool(&self, mut state: VMState, dest: Slot, value: bool) -> VMState {
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::boolean(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_int(&self, mut state: VMState, dest: Slot, value: i64) -> VMState {
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::integer(value)),
        );
        Self::advance_pc(state)
    }

    fn execute_ldc_unit(&self, mut state: VMState, dest: Slot) -> VMState {
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::unit()),
        );
        Self::advance_pc(state)
    }

    fn execute_mov(&self, mut state: VMState, dest: Slot, src: Slot) -> Result<VMState, String> {
        let value = Self::read_slot(&state, src)?;
        Self::write_slot(&mut state, dest, value);
        Ok(Self::advance_pc(state))
    }

    fn execute_switch(
        &self,
        state: VMState,
        var: Slot,
        offsets: &[i32],
    ) -> Result<VMState, String> {
        let value = Self::read_slot(&state, var)?;

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
        state: VMState,
        var: Slot,
    ) -> Result<(VMState, ExpressionResult), String> {
        let result = Self::read_slot(&state, var)?;
        Ok((state, result))
    }

    async fn execute_call(
        &self,
        state: VMState,
        function_name: &DefinitionPath,
        params: &[Slot],
        dest: Slot,
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
        state: VMState,
        func: Arc<dyn ExecutableFunction>,
        display_name: &str,
        params: &[Slot],
        dest: Slot,
    ) -> Result<VMState, String> {
        let args = params
            .iter()
            .map(|s| Self::read_slot(&state, *s))
            .collect::<Result<Vec<_>, _>>()?;
        self.invoke_function_with_args(state, func, display_name, args, dest)
            .await
    }

    async fn invoke_function_with_args(
        &self,
        mut state: VMState,
        func: Arc<dyn ExecutableFunction>,
        display_name: &str,
        args: Vec<ExpressionResult>,
        dest: Slot,
    ) -> Result<VMState, String> {
        let function_params = func.parameters();

        let evaluated_parameters: Vec<ExpressionParameter> = args
            .iter()
            .zip(function_params.iter())
            .map(|(arg, param)| ExpressionParameter::new(param.name.clone(), arg.value.clone()))
            .collect();

        let mut child_context = state.context.create_child();

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

        Self::write_slot(&mut state, dest, result_with_metadata);
        Ok(Self::advance_pc(state))
    }

    async fn execute_external_call(
        &self,
        state: VMState,
        function_name: &DefinitionPath,
        params: &[Slot],
        dest: Slot,
    ) -> Result<VMState, String> {
        static CALL_COUNTER: AtomicU64 = AtomicU64::new(0);
        let call_id = CALL_COUNTER.fetch_add(1, Ordering::Relaxed).to_string();
        let name_str = function_name.to_string();
        let lookup_name = function_name.last_name().to_string();

        let resolved_params: HashMap<String, ExpressionValue> = params
            .iter()
            .filter_map(|slot| {
                Self::read_slot(&state, *slot)
                    .ok()
                    .map(|r| (format!("s{}", slot.0), r.value))
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

        let result = Self::read_slot(&state, dest)?;

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

    fn execute_ctx_event(&self, mut state: VMState, var: Slot) -> Result<VMState, String> {
        let expr_result = Self::read_slot(&state, var)?;

        state.context.add_event(
            expr_result.value.clone(),
            expr_result.name.clone(),
            expr_result.params.clone(),
        );
        Ok(Self::advance_pc(state))
    }

    fn execute_ctx_child(&self, state: VMState) -> VMState {
        let child_context = state.context.create_child();
        let new_state = VMState {
            pc: state.pc,
            context: child_context,
            frame: state.frame,
        };
        Self::advance_pc(new_state)
    }

    fn execute_ctx_restore(&self, state: VMState) -> Result<VMState, String> {
        let parent_context = state.context.restore_parent()?;
        let new_state = VMState {
            pc: state.pc,
            context: parent_context,
            frame: state.frame,
        };
        Ok(Self::advance_pc(new_state))
    }

    fn execute_meta_function(
        &self,
        mut state: VMState,
        function_name: &DefinitionPath,
        dest: Slot,
    ) -> Result<VMState, String> {
        let func = self
            .runtime
            .get_bytecode_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let name_str = function_name.to_string();
        let metadata =
            ExpressionValue::metadata(&name_str, func.documentation().map(|s| s.to_string()));

        Self::write_slot(&mut state, dest, ExpressionResult::new(metadata));
        Ok(Self::advance_pc(state))
    }

    fn execute_load_module(
        &self,
        mut state: VMState,
        name: &DefinitionPath,
        param_slots: &[Slot],
        dest: Slot,
    ) -> Result<VMState, String> {
        let params: Vec<ExpressionValue> = param_slots
            .iter()
            .map(|s| Ok(Self::read_slot(&state, *s)?.value))
            .collect::<Result<_, String>>()?;
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::Module {
                path: name.clone(),
                params,
            }),
        );
        Ok(Self::advance_pc(state))
    }

    fn execute_list_create(
        &self,
        mut state: VMState,
        dest: Slot,
        element_slots: &[Slot],
    ) -> Result<VMState, String> {
        let elements: Vec<ExpressionValue> = element_slots
            .iter()
            .map(|slot| Ok(Self::read_slot(&state, *slot)?.value))
            .collect::<Result<_, String>>()?;

        let list_value = ExpressionValue::from_elements(elements)?;
        Self::write_slot(&mut state, dest, ExpressionResult::new(list_value));
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_placeholder(
        &self,
        mut state: VMState,
        dest: Slot,
        param_name: &str,
        param_type: &structured_agent_runtime::Type,
    ) -> Result<VMState, String> {
        let (value, thinking) = state
            .context
            .runtime()
            .engine()
            .request(
                &state.context,
                &FillParameterEvent {
                    param_name: param_name.to_string(),
                    param_type: param_type.clone(),
                },
            )
            .await?;
        if let Some(t) = thinking {
            state.context.add_thinking_event(t);
        }

        Self::write_slot(&mut state, dest, ExpressionResult::new(value));
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_select(
        &self,
        mut state: VMState,
        metadata_slots: &[Slot],
        dest: Slot,
    ) -> Result<VMState, String> {
        let mut metadata_values = Vec::new();

        for slot in metadata_slots {
            let value = Self::read_slot(&state, *slot)?;
            if value.value.type_name() != "Metadata" {
                return Err(format!(
                    "Expected Metadata value in slot {}, got {}",
                    slot.0,
                    value.value.type_name()
                ));
            }
            metadata_values.push(value.value.clone());
        }

        let (value, thinking) = state
            .context
            .runtime()
            .engine()
            .request(
                &state.context,
                &SelectEvent {
                    options: metadata_values,
                },
            )
            .await?;
        if let Some(t) = thinking {
            state.context.add_thinking_event(t);
        }
        let selected_index = value
            .as_integer()
            .map_err(|e| format!("Expected integer selection: {}", e))?
            as usize;

        let result = ExpressionResult::new(ExpressionValue::string(selected_index.to_string()));

        Self::write_slot(&mut state, dest, result);
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_generate(
        &self,
        mut state: VMState,
        dest: Slot,
        return_type: &structured_agent_runtime::Type,
    ) -> Result<VMState, String> {
        let (value, thinking) = state
            .context
            .runtime()
            .engine()
            .request(
                &state.context,
                &TypedEvent {
                    return_type: return_type.clone(),
                },
            )
            .await?;
        if let Some(t) = thinking {
            state.context.add_thinking_event(t);
        }

        state
            .context
            .agent_handle()
            .publish(AgentMessageContent::String(value.format_for_llm()));

        Self::write_slot(&mut state, dest, ExpressionResult::new(value));
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

    fn read_slot(state: &VMState, slot: Slot) -> Result<ExpressionResult, String> {
        state
            .frame
            .get(slot.0 as usize)
            .and_then(|v| v.clone())
            .ok_or_else(|| format!("Slot {} not initialized", slot.0))
    }

    fn write_slot(state: &mut VMState, slot: Slot, value: ExpressionResult) {
        if let Some(entry) = state.frame.get_mut(slot.0 as usize) {
            *entry = Some(value);
        }
    }

    fn branch_if_bool(
        state: VMState,
        var: Slot,
        offset: i32,
        expected: bool,
    ) -> Result<VMState, String> {
        let value = Self::read_slot(&state, var)?;

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

    async fn execute_native_call(
        &self,
        mut state: VMState,
        f: &NativeFnPtr,
        params: &[Slot],
        dest: Slot,
    ) -> Result<VMState, String> {
        let args: Vec<ExpressionValue> = params
            .iter()
            .map(|s| Self::read_slot(&state, *s).map(|r| r.value))
            .collect::<Result<Vec<_>, _>>()?;
        let agent_handle = state.context.agent_handle().clone();
        let result = f.call(args, agent_handle).await?;
        Self::write_slot(&mut state, dest, ExpressionResult::new(result));
        Ok(Self::advance_pc(state))
    }

    async fn execute_indirect_call(
        &self,
        state: VMState,
        module_param: Slot,
        fn_name: &DefinitionPath,
        params: &[Slot],
        dest: Slot,
    ) -> Result<VMState, String> {
        let module_val = Self::read_slot(&state, module_param)?;
        let (module_path, module_params) = match &module_val.value {
            ExpressionValue::Module { path, params } => (path.clone(), params.clone()),
            _ => return Err("CallIndirect: expected Module".to_string()),
        };
        let function_name = DefinitionPath::for_function(module_path, fn_name.last_name());
        let func = self
            .runtime
            .get_bytecode_function(&function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;
        let mut args: Vec<ExpressionResult> = module_params
            .into_iter()
            .map(ExpressionResult::new)
            .collect();
        for slot in params {
            args.push(Self::read_slot(&state, *slot)?);
        }
        self.invoke_function_with_args(state, func, &function_name.to_string(), args, dest)
            .await
    }

    fn execute_struct_new(
        &self,
        mut state: VMState,
        dest: Slot,
        fields: &[(String, Slot)],
    ) -> Result<VMState, String> {
        let field_values: Vec<(&str, ExpressionValue)> = fields
            .iter()
            .map(|(name, src)| {
                let val = Self::read_slot(&state, *src)?;
                Ok((name.as_str(), val.value.clone()))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let struct_value = ExpressionValue::struct_value(field_values);
        Self::write_slot(&mut state, dest, ExpressionResult::new(struct_value));
        Ok(Self::advance_pc(state))
    }

    fn execute_struct_get(
        &self,
        mut state: VMState,
        dest: Slot,
        src: Slot,
        field: &str,
    ) -> Result<VMState, String> {
        let src_val = Self::read_slot(&state, src)?;
        let field_value = src_val.value.get_struct_field(field)?;
        Self::write_slot(&mut state, dest, ExpressionResult::new(field_value));
        Ok(Self::advance_pc(state))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use structured_agent_il::Instruction;
    use structured_agent_il::slot::Slot;
    use structured_agent_interpreter_runtime::{Context, ExpressionValue, RuntimeService};
    use structured_agent_runtime::{DefinitionPath, NativeFnPtr};

    use crate::vm::VM;

    struct NoopRuntime;

    impl RuntimeService for NoopRuntime {
        fn get_native_function(
            &self,
            _name: &str,
        ) -> Option<Arc<dyn structured_agent_interpreter_runtime::ExecutableFunction>> {
            None
        }
        fn get_bytecode_function(
            &self,
            _name: &DefinitionPath,
        ) -> Option<Arc<dyn structured_agent_interpreter_runtime::ExecutableFunction>> {
            None
        }
        fn engine(&self) -> &dyn structured_agent_interpreter_runtime::LanguageEngine {
            unimplemented!()
        }
        fn type_to_arrow_datatype(
            &self,
            _ty: &structured_agent_runtime::Type,
        ) -> arrow::datatypes::DataType {
            arrow::datatypes::DataType::Null
        }
        fn get_struct(
            &self,
            _type_name: &DefinitionPath,
        ) -> Option<Vec<(String, structured_agent_runtime::Type)>> {
            None
        }
        fn get_struct_with_args(
            &self,
            _type_name: &DefinitionPath,
            _args: &[structured_agent_runtime::Type],
        ) -> Option<Vec<(String, structured_agent_runtime::Type)>> {
            None
        }
    }

    fn make_context() -> Context {
        Context::with_runtime(Arc::new(NoopRuntime))
    }

    #[tokio::test]
    async fn execute_call_native_writes_result_to_dest() {
        let f = NativeFnPtr::new(|_, _| Box::pin(async { Ok(ExpressionValue::string("ok")) }));

        let instructions = vec![
            Instruction::LdcStr {
                dest: Slot(1),
                value: "input".to_string(),
            },
            Instruction::CallNative {
                f,
                params: vec![Slot(1)],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ];

        let vm = VM::new(Arc::new(NoopRuntime));
        let frame = vec![None, None];
        let context = make_context();
        let (_, result) = vm.execute(&instructions, context, frame).await.unwrap();
        assert_eq!(result.value, ExpressionValue::string("ok"));
    }
}
