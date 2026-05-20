use std::collections::HashMap;

use std::sync::Arc;

use structured_agent_il::slot::Slot;
use structured_agent_il::{BytecodeRef, Instruction};
use structured_agent_interpreter_runtime::{
    AgentMessageContent, Context, ExecutableFunction, ExpressionParameter, ExpressionResult,
    ExpressionValue, FillParameterEvent, RuntimeService, SelectEvent, TypedEvent,
};
use structured_agent_runtime::{
    ActorRef, AgentHandle, DefinitionPath, DefinitionSegment, NativeFnPtr,
};

struct CallFrame {
    instructions: Arc<[Instruction]>,
    pc: usize,
    slots: Vec<Option<ExpressionResult>>,
    dest: Slot,
    display_name: String,
    evaluated_parameters: Vec<ExpressionParameter>,
    needs_context_restore: bool,
}

pub enum VMOutcome {
    Complete(Context, ExpressionResult),
    Yielded(VMState),
}

pub struct VMState {
    call_stack: Vec<CallFrame>,
    context: Context,
}

impl VMState {
    pub fn agent_handle(&self) -> &AgentHandle {
        self.context.agent_handle()
    }
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
        match self.execute_outcome(instructions, context, frame).await? {
            VMOutcome::Complete(ctx, result) => Ok((ctx, result)),
            VMOutcome::Yielded(_) => {
                Err("actor yield not supported in non-actor context".to_string())
            }
        }
    }

    pub async fn execute_outcome(
        &self,
        instructions: &[Instruction],
        context: Context,
        frame: Vec<Option<ExpressionResult>>,
    ) -> Result<VMOutcome, String> {
        let instructions_arc: Arc<[Instruction]> = Arc::from(instructions);
        let initial_frame = CallFrame {
            instructions: instructions_arc,
            pc: 0,
            slots: frame,
            dest: Slot(0),
            display_name: String::new(),
            evaluated_parameters: vec![],
            needs_context_restore: false,
        };
        let state = VMState {
            call_stack: vec![initial_frame],
            context,
        };
        self.run_dispatch_loop(state).await
    }

    pub async fn resume_outcome(&self, state: VMState) -> Result<VMOutcome, String> {
        self.run_dispatch_loop(state).await
    }

    async fn run_dispatch_loop(&self, mut state: VMState) -> Result<VMOutcome, String> {
        loop {
            let instruction = {
                let frame = state
                    .call_stack
                    .last()
                    .ok_or_else(|| "Call stack empty".to_string())?;
                if frame.pc >= frame.instructions.len() {
                    return Err("PC out of bounds".to_string());
                }
                frame.instructions[frame.pc].clone()
            };

            state = match instruction {
                Instruction::Nop => Self::advance_pc(state),
                Instruction::LdcStr { dest, value } => self.execute_ldc_str(state, dest, &value),
                Instruction::LdcBool { dest, value } => self.execute_ldc_bool(state, dest, value),
                Instruction::LdcInt { dest, value } => self.execute_ldc_int(state, dest, value),
                Instruction::LdcUnit { dest } => self.execute_ldc_unit(state, dest),
                Instruction::Mov { dest, src } => self.execute_mov(state, dest, src)?,
                Instruction::Br { offset } => Self::branch(state, offset as usize),
                Instruction::BrFalse { var, offset } => {
                    Self::branch_if_bool(state, var, offset, false)?
                }
                Instruction::BrTrue { var, offset } => {
                    Self::branch_if_bool(state, var, offset, true)?
                }
                Instruction::Switch { var, offsets } => {
                    self.execute_switch(state, var, &offsets)?
                }
                Instruction::Ret { var } => {
                    let result = Self::read_slot(&state, var)?;
                    let frame = state.call_stack.pop().unwrap();
                    if frame.needs_context_restore {
                        state.context = state.context.restore_parent()?;
                    }
                    if state.call_stack.is_empty() {
                        return Ok(VMOutcome::Complete(state.context, result));
                    }
                    let result_with_meta = ExpressionResult {
                        name: Some(frame.display_name),
                        params: Some(frame.evaluated_parameters),
                        value: result.value,
                    };
                    state.call_stack.last_mut().unwrap().slots[frame.dest.0 as usize] =
                        Some(result_with_meta);
                    state
                }
                Instruction::Snapshot => {
                    return Err("snapshot/durable-yield not yet implemented".to_string());
                }
                Instruction::ActorYield => return Ok(VMOutcome::Yielded(Self::advance_pc(state))),
                Instruction::Spawn {
                    module_path,
                    key_slot,
                    dest,
                } => {
                    self.execute_spawn(state, module_path.clone(), key_slot, dest)
                        .await?
                }
                Instruction::CallActor {
                    actor_slot,
                    fn_name,
                    params,
                    dest,
                } => {
                    self.execute_call_actor(state, actor_slot, fn_name, params, dest)
                        .await?
                }
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
                    self.execute_external_call(state, &function_name, &params, dest)
                        .await?
                }
                Instruction::LoadModule { name, dest } => {
                    self.execute_load_module(state, &name, dest)?
                }
                Instruction::CtxEvent { var } => self.execute_ctx_event(state, var)?,
                Instruction::CtxChild => self.execute_ctx_child(state),
                Instruction::CtxRestore => self.execute_ctx_restore(state)?,
                Instruction::MetaFunction {
                    function_name,
                    dest,
                } => self.execute_meta_function(state, &function_name, dest)?,
                Instruction::ListCreate { dest, elements } => {
                    self.execute_list_create(state, dest, &elements)?
                }
                Instruction::StrConcat { dest, parts } => {
                    self.execute_str_concat(state, dest, &parts)?
                }
                Instruction::LlmPlaceholder {
                    dest,
                    param_name,
                    function_name,
                    param_type,
                } => {
                    self.execute_llm_placeholder(
                        state,
                        dest,
                        &function_name,
                        &param_name,
                        &param_type,
                    )
                    .await?
                }
                Instruction::LlmSelect {
                    metadata_vars,
                    dest,
                } => self.execute_llm_select(state, &metadata_vars, dest).await?,
                Instruction::LlmGenerate { dest, return_type } => {
                    self.execute_llm_generate(state, dest, &return_type).await?
                }
                Instruction::StructNew {
                    dest,
                    struct_name,
                    fields,
                } => self.execute_struct_new(state, dest, &struct_name, &fields)?,
                Instruction::StructGet { dest, src, field } => {
                    self.execute_struct_get(state, dest, src, &field)?
                }
                Instruction::CallNative { f, params, dest } => {
                    self.execute_native_call(state, &f, &params, dest).await?
                }
                Instruction::MatchType { src, dest, variant } => {
                    self.execute_match_type(state, src, dest, &variant)?
                }
                Instruction::CallVirtual {
                    module_slot,
                    method,
                    params,
                    dest,
                } => {
                    self.execute_virtual_call(state, module_slot, &method, params, dest)
                        .await?
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

    async fn execute_call(
        &self,
        state: VMState,
        function_name: DefinitionPath,
        params: Vec<Slot>,
        dest: Slot,
    ) -> Result<VMState, String> {
        let body = self
            .runtime
            .get_bytecode_ref(&function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;
        let display_name = function_name.to_string();
        let args: Vec<ExpressionResult> = params
            .iter()
            .map(|s| Self::read_slot(&state, *s))
            .collect::<Result<Vec<_>, _>>()?;
        let state = Self::advance_pc(state);
        self.push_bytecode_frame(state, body, display_name, args, dest)
    }

    fn push_bytecode_frame(
        &self,
        mut state: VMState,
        body: BytecodeRef,
        display_name: String,
        args: Vec<ExpressionResult>,
        dest: Slot,
    ) -> Result<VMState, String> {
        let instructions: Arc<[Instruction]> = body.instructions.into();
        let slot_count = body.slot_table.len();
        let evaluated_parameters: Vec<ExpressionParameter> = args
            .iter()
            .zip(body.parameters.iter())
            .map(|(arg, param)| ExpressionParameter::new(param.name.clone(), arg.value.clone()))
            .collect();
        let mut child_context = state.context.create_child();
        child_context.add_event(
            ExpressionValue::string(format!("## {}", display_name)),
            None,
            None,
        );
        state.context = child_context;
        let mut callee_frame = vec![None; slot_count];
        for (i, arg) in args.into_iter().enumerate() {
            callee_frame[i + 1] = Some(arg);
        }
        state.call_stack.push(CallFrame {
            instructions,
            pc: 0,
            slots: callee_frame,
            dest,
            display_name,
            evaluated_parameters,
            needs_context_restore: true,
        });
        Ok(state)
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
        let call_id = structured_agent_runtime::next_call_id();
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

    fn execute_ctx_child(&self, mut state: VMState) -> VMState {
        state.context = state.context.create_child();
        Self::advance_pc(state)
    }

    fn execute_ctx_restore(&self, mut state: VMState) -> Result<VMState, String> {
        state.context = state.context.restore_parent()?;
        Ok(Self::advance_pc(state))
    }

    fn execute_meta_function(
        &self,
        mut state: VMState,
        function_name: &DefinitionPath,
        dest: Slot,
    ) -> Result<VMState, String> {
        let name_str = function_name.to_string();
        let documentation = if let Some(body) = self.runtime.get_bytecode_ref(function_name) {
            body.documentation
        } else if let Some(func) = self.runtime.get_native_function(function_name.last_name()) {
            func.documentation().map(|s| s.to_string())
        } else if matches!(function_name.segments.last(), DefinitionSegment::Type(_)) {
            self.runtime.get_type_documentation(function_name)
        } else {
            return Err(format!("Definition not found: {}", function_name));
        };
        let metadata = ExpressionValue::metadata(&name_str, documentation);
        Self::write_slot(&mut state, dest, ExpressionResult::new(metadata));
        Ok(Self::advance_pc(state))
    }

    fn execute_load_module(
        &self,
        mut state: VMState,
        name: &DefinitionPath,
        dest: Slot,
    ) -> Result<VMState, String> {
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::module(name.clone())),
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

    fn execute_str_concat(
        &self,
        mut state: VMState,
        dest: Slot,
        parts: &[Slot],
    ) -> Result<VMState, String> {
        let concatenated: String = parts
            .iter()
            .map(|slot| Ok(Self::read_slot(&state, *slot)?.value.format_for_llm()))
            .collect::<Result<Vec<String>, String>>()?
            .join("");
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::string(&concatenated)),
        );
        Ok(Self::advance_pc(state))
    }

    async fn execute_llm_placeholder(
        &self,
        mut state: VMState,
        dest: Slot,
        function_name: &str,
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
                    function_name: function_name.to_string(),
                    param_name: param_name.to_string(),
                    param_type: param_type.clone(),
                },
            )
            .await?;
        if let Some(t) = thinking {
            state
                .context
                .agent_handle()
                .publish(AgentMessageContent::Thinking {
                    content: t.content.clone(),
                });
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
        let option_count = metadata_values.len();
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
            state
                .context
                .agent_handle()
                .publish(AgentMessageContent::Thinking {
                    content: t.content.clone(),
                });
            state.context.add_thinking_event(t);
        }
        let selected_index = value
            .as_integer()
            .map_err(|e| format!("Expected integer selection: {}", e))?
            as usize;
        if selected_index >= option_count {
            return Err(format!(
                "Selected index {} is out of bounds for {} options",
                selected_index, option_count
            ));
        }
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
        let call_id = structured_agent_runtime::next_call_id();

        let display_name = state
            .call_stack
            .last()
            .map(|f| f.display_name.clone())
            .unwrap_or_default();

        let params = state
            .call_stack
            .last()
            .map(|f| {
                f.evaluated_parameters
                    .iter()
                    .map(|p| (p.name.clone(), p.value.clone()))
                    .collect()
            })
            .unwrap_or_default();

        state
            .context
            .agent_handle()
            .publish(AgentMessageContent::ToolCallStarted {
                tool_name: display_name.clone(),
                call_id: call_id.clone(),
                params,
            });

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
            state
                .context
                .agent_handle()
                .publish(AgentMessageContent::Thinking {
                    content: t.content.clone(),
                });
            state.context.add_thinking_event(t);
        }
        state
            .context
            .agent_handle()
            .publish(AgentMessageContent::ToolCallFinished {
                tool_name: display_name,
                call_id,
                result: value.clone(),
            });
        Self::write_slot(&mut state, dest, ExpressionResult::new(value));
        Ok(Self::advance_pc(state))
    }

    fn advance_pc(mut state: VMState) -> VMState {
        state.call_stack.last_mut().unwrap().pc += 1;
        state
    }

    fn branch(mut state: VMState, offset: usize) -> VMState {
        state.call_stack.last_mut().unwrap().pc = offset;
        state
    }

    fn read_slot(state: &VMState, slot: Slot) -> Result<ExpressionResult, String> {
        state
            .call_stack
            .last()
            .and_then(|f| f.slots.get(slot.0 as usize))
            .and_then(|v| v.clone())
            .ok_or_else(|| format!("Slot {} not initialized", slot.0))
    }

    fn write_slot(state: &mut VMState, slot: Slot, value: ExpressionResult) {
        if let Some(frame) = state.call_stack.last_mut()
            && let Some(entry) = frame.slots.get_mut(slot.0 as usize)
        {
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

    async fn execute_virtual_call(
        &self,
        state: VMState,
        module_slot: Slot,
        method: &str,
        params: Vec<Slot>,
        dest: Slot,
    ) -> Result<VMState, String> {
        let module_val = Self::read_slot(&state, module_slot)?;
        let impl_key = module_val.value.as_module()?;
        let function_name = DefinitionPath::for_function(impl_key.clone(), method);
        self.execute_call(state, function_name, params, dest).await
    }

    fn execute_struct_new(
        &self,
        mut state: VMState,
        dest: Slot,
        struct_name: &DefinitionPath,
        fields: &[(String, Slot)],
    ) -> Result<VMState, String> {
        let field_values: Vec<(&str, ExpressionValue)> = fields
            .iter()
            .map(|(name, src)| {
                let val = Self::read_slot(&state, *src)?;
                Ok((name.as_str(), val.value.clone()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let struct_value = ExpressionValue::named_struct_value(struct_name.clone(), field_values);
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

    fn execute_match_type(
        &self,
        mut state: VMState,
        src: Slot,
        dest: Slot,
        variant: &DefinitionPath,
    ) -> Result<VMState, String> {
        let value = Self::read_slot(&state, src)?;
        let matched = value.value.type_path == *variant;
        Self::write_slot(
            &mut state,
            dest,
            ExpressionResult::new(ExpressionValue::boolean(matched)),
        );
        Ok(Self::advance_pc(state))
    }

    async fn execute_spawn(
        &self,
        mut state: VMState,
        module_path: DefinitionPath,
        key_slot: Slot,
        dest: Slot,
    ) -> Result<VMState, String> {
        let key_val = Self::read_slot(&state, key_slot)?;
        let key = key_val.value.as_string()?;
        let registry_key = format!("{}:{}", module_path, key);
        let actor_id = registry_key.clone();
        let handle = state.context.agent_handle().clone();
        let actor_ref =
            self.runtime
                .actor_registry()
                .get_or_create(registry_key, module_path, |rx| {
                    let actor_handle = handle.with_actor_id(actor_id);
                    let actor_ctx =
                        Context::with_runtime_and_handle(self.runtime.clone(), actor_handle);
                    self.runtime.spawn_actor(rx, actor_ctx);
                });
        let type_path = DefinitionPath::for_type(actor_ref.module_path.clone(), "ActorRef");
        let actor_ref_result = ExpressionResult::new(ExpressionValue::from_runtime_value(
            type_path,
            Arc::new(actor_ref),
        ));
        state = Self::advance_pc(state);
        Self::write_slot(&mut state, dest, actor_ref_result);
        Ok(state)
    }

    async fn execute_call_actor(
        &self,
        mut state: VMState,
        actor_slot: Slot,
        fn_name: DefinitionPath,
        params: Vec<Slot>,
        dest: Slot,
    ) -> Result<VMState, String> {
        let actor_val = Self::read_slot(&state, actor_slot)?;
        let actor_ref = actor_val
            .value
            .downcast_clone::<ActorRef>()
            .map_err(|_| "CallActor: expected ActorRef".to_string())?;
        let mut args = Vec::new();
        for slot in &params {
            args.push(Self::read_slot(&state, *slot)?);
        }
        let result_value = actor_ref.call(fn_name, args).await?;
        let result = ExpressionResult::new(result_value);
        state = Self::advance_pc(state);
        Self::write_slot(&mut state, dest, result);
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_il::slot::{Slot, SlotKind, SlotTable};
    use structured_agent_il::{BytecodeRef, Instruction};
    use structured_agent_interpreter_runtime::{
        Context, ExecutableFunction, ExpressionResult, ExpressionValue, RuntimeService,
    };
    use structured_agent_runtime::{DefinitionPath, NativeFnPtr, Parameter, Type};

    use crate::vm::{VM, VMOutcome};

    struct NoopRuntime;

    impl RuntimeService for NoopRuntime {
        fn get_native_function(&self, _name: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn get_bytecode_ref(&self, _name: &DefinitionPath) -> Option<BytecodeRef> {
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

    struct TestRuntime {
        functions: HashMap<DefinitionPath, BytecodeRef>,
    }

    impl TestRuntime {
        fn new(functions: HashMap<DefinitionPath, BytecodeRef>) -> Self {
            Self { functions }
        }
    }

    impl RuntimeService for TestRuntime {
        fn get_native_function(&self, _name: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn get_bytecode_ref(&self, name: &DefinitionPath) -> Option<BytecodeRef> {
            self.functions.get(name).cloned()
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

    fn make_context_with_runtime(runtime: Arc<dyn RuntimeService>) -> Context {
        Context::with_runtime(runtime)
    }

    fn make_bytecode_ref(
        instructions: Vec<Instruction>,
        slot_count: usize,
        params: Vec<Parameter>,
    ) -> BytecodeRef {
        let mut slot_table = SlotTable::new();
        for i in 0..slot_count {
            slot_table.push(SlotKind::Temp, format!("s{}", i));
        }
        BytecodeRef {
            instructions,
            labels: HashMap::new(),
            parameters: params,
            return_type: Type::string(),
            documentation: None,
            slot_table,
        }
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

    #[tokio::test]
    async fn execute_match_type_matching_writes_true() {
        let instructions = vec![
            Instruction::Mov {
                dest: Slot(1),
                src: Slot(1),
            },
            Instruction::MatchType {
                src: Slot(1),
                dest: Slot(0),
                variant: DefinitionPath::root()
                    .with_module("prelude".to_string())
                    .with_type("Image".to_string()),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(Arc::new(NoopRuntime));
        let frame = vec![
            None,
            Some(ExpressionResult::new(ExpressionValue::image(
                "image/png",
                vec![],
            ))),
        ];
        let context = make_context();
        let (_, result) = vm.execute(&instructions, context, frame).await.unwrap();
        assert_eq!(result.value, ExpressionValue::boolean(true));
    }

    #[tokio::test]
    async fn execute_match_type_non_matching_writes_false() {
        let instructions = vec![
            Instruction::Mov {
                dest: Slot(1),
                src: Slot(1),
            },
            Instruction::MatchType {
                src: Slot(1),
                dest: Slot(0),
                variant: DefinitionPath::root()
                    .with_module("prelude".to_string())
                    .with_type("Image".to_string()),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(Arc::new(NoopRuntime));
        let frame = vec![
            None,
            Some(ExpressionResult::new(ExpressionValue::string(
                "not an image",
            ))),
        ];
        let context = make_context();
        let (_, result) = vm.execute(&instructions, context, frame).await.unwrap();
        assert_eq!(result.value, ExpressionValue::boolean(false));
    }

    #[tokio::test]
    async fn execute_call_bytecode_returns_result() {
        let callee_path = DefinitionPath::for_function(DefinitionPath::root(), "callee");

        let callee_ref = make_bytecode_ref(
            vec![
                Instruction::LdcStr {
                    dest: Slot(1),
                    value: "hello".to_string(),
                },
                Instruction::Ret { var: Slot(1) },
            ],
            2,
            vec![],
        );

        let mut functions = HashMap::new();
        functions.insert(callee_path.clone(), callee_ref);

        let runtime = Arc::new(TestRuntime::new(functions));
        let context = make_context_with_runtime(runtime.clone());

        let caller_instructions = vec![
            Instruction::CallBytecode {
                function_name: callee_path,
                params: vec![],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ];

        let vm = VM::new(runtime);
        let (_, result) = vm
            .execute(&caller_instructions, context, vec![None])
            .await
            .unwrap();

        assert_eq!(result.value, ExpressionValue::string("hello"));
    }

    #[tokio::test]
    async fn execute_nested_call_bytecode_returns_result() {
        let inner_path = DefinitionPath::for_function(DefinitionPath::root(), "inner");
        let helper_path = DefinitionPath::for_function(DefinitionPath::root(), "helper");

        let inner_ref = make_bytecode_ref(
            vec![
                Instruction::LdcStr {
                    dest: Slot(1),
                    value: "nested".to_string(),
                },
                Instruction::Ret { var: Slot(1) },
            ],
            2,
            vec![],
        );

        let helper_ref = make_bytecode_ref(
            vec![
                Instruction::CallBytecode {
                    function_name: inner_path.clone(),
                    params: vec![],
                    dest: Slot(1),
                },
                Instruction::Ret { var: Slot(1) },
            ],
            2,
            vec![],
        );

        let mut functions = HashMap::new();
        functions.insert(inner_path, inner_ref);
        functions.insert(helper_path.clone(), helper_ref);

        let runtime = Arc::new(TestRuntime::new(functions));
        let context = make_context_with_runtime(runtime.clone());

        let main_instructions = vec![
            Instruction::CallBytecode {
                function_name: helper_path,
                params: vec![],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ];

        let vm = VM::new(runtime);
        let (_, result) = vm
            .execute(&main_instructions, context, vec![None])
            .await
            .unwrap();

        assert_eq!(result.value, ExpressionValue::string("nested"));
    }

    #[tokio::test]
    async fn actor_yield_in_non_actor_context_returns_error() {
        let instructions = vec![Instruction::ActorYield];
        let vm = VM::new(Arc::new(NoopRuntime));
        let context = make_context();
        let frame = vec![];
        let result = vm.execute(&instructions, context, frame).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("actor yield"));
    }

    #[tokio::test]
    async fn actor_yield_via_execute_outcome_returns_yielded() {
        let instructions = vec![Instruction::ActorYield];
        let vm = VM::new(Arc::new(NoopRuntime));
        let context = make_context();
        let frame = vec![];
        let outcome = vm
            .execute_outcome(&instructions, context, frame)
            .await
            .unwrap();
        assert!(matches!(outcome, VMOutcome::Yielded(_)));
    }

    #[tokio::test]
    async fn execute_str_concat_concatenates_string_slots() {
        let instructions = vec![
            Instruction::LdcStr {
                dest: Slot(1),
                value: "hello ".to_string(),
            },
            Instruction::LdcStr {
                dest: Slot(2),
                value: "world".to_string(),
            },
            Instruction::StrConcat {
                dest: Slot(0),
                parts: vec![Slot(1), Slot(2)],
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(Arc::new(NoopRuntime));
        let context = make_context();
        let frame = vec![None, None, None];
        let (_, result) = vm.execute(&instructions, context, frame).await.unwrap();
        assert_eq!(result.value, ExpressionValue::string("hello world"));
    }

    #[tokio::test]
    async fn execute_str_concat_empty_parts_produces_empty_string() {
        let instructions = vec![
            Instruction::StrConcat {
                dest: Slot(0),
                parts: vec![],
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(Arc::new(NoopRuntime));
        let context = make_context();
        let frame = vec![None];
        let (_, result) = vm.execute(&instructions, context, frame).await.unwrap();
        assert_eq!(result.value, ExpressionValue::string(""));
    }

    struct EngineRuntime {
        engine: Arc<structured_agent_interpreter_runtime::PrintEngine>,
    }

    impl RuntimeService for EngineRuntime {
        fn get_native_function(&self, _name: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn get_bytecode_ref(&self, _name: &DefinitionPath) -> Option<BytecodeRef> {
            None
        }
        fn engine(&self) -> &dyn structured_agent_interpreter_runtime::LanguageEngine {
            self.engine.as_ref()
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

    fn make_engine_context_with_handle(
        handle: structured_agent_runtime::AgentHandle,
    ) -> (Arc<EngineRuntime>, Context) {
        let runtime = Arc::new(EngineRuntime {
            engine: Arc::new(structured_agent_interpreter_runtime::PrintEngine {}),
        });
        let context = Context::with_runtime_and_handle(runtime.clone(), handle);
        (runtime, context)
    }

    #[tokio::test]
    async fn generate_string_publishes_tool_call_started_and_finished() {
        use structured_agent_runtime::{AgentHandle, AgentMessageContent};
        let handle = AgentHandle::detached();
        let mut subscriber = handle.subscribe();
        let (runtime, context) = make_engine_context_with_handle(handle);

        let instructions = vec![
            Instruction::LlmGenerate {
                dest: Slot(0),
                return_type: Type::string(),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(runtime);
        let frame = vec![None];
        vm.execute(&instructions, context, frame).await.unwrap();

        let msg1 = subscriber.try_recv().unwrap();
        assert!(matches!(
            msg1.content,
            AgentMessageContent::ToolCallStarted { .. }
        ));

        let msg2 = subscriber.try_recv().unwrap();
        assert!(matches!(
            msg2.content,
            AgentMessageContent::ToolCallFinished { .. }
        ));
    }

    #[tokio::test]
    async fn generate_boolean_publishes_tool_call_started_and_finished() {
        use structured_agent_runtime::{AgentHandle, AgentMessageContent};
        let handle = AgentHandle::detached();
        let mut subscriber = handle.subscribe();
        let (runtime, context) = make_engine_context_with_handle(handle);

        let instructions = vec![
            Instruction::LlmGenerate {
                dest: Slot(0),
                return_type: Type::boolean(),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(runtime);
        let frame = vec![None];
        vm.execute(&instructions, context, frame).await.unwrap();

        let msg1 = subscriber.try_recv().unwrap();
        assert!(matches!(
            msg1.content,
            AgentMessageContent::ToolCallStarted { .. }
        ));

        let msg2 = subscriber.try_recv().unwrap();
        assert!(matches!(
            msg2.content,
            AgentMessageContent::ToolCallFinished { .. }
        ));
    }

    #[tokio::test]
    async fn resume_after_yield_completes() {
        let instructions = vec![
            Instruction::ActorYield,
            Instruction::LdcUnit { dest: Slot(0) },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(Arc::new(NoopRuntime));
        let context = make_context();
        let frame = vec![None];
        let outcome = vm
            .execute_outcome(&instructions, context, frame)
            .await
            .unwrap();
        let VMOutcome::Yielded(state) = outcome else {
            panic!("expected Yielded")
        };
        let outcome2 = vm.resume_outcome(state).await.unwrap();
        assert!(matches!(outcome2, VMOutcome::Complete(_, _)));
    }

    struct ThinkingEngine;

    #[async_trait::async_trait]
    impl structured_agent_interpreter_runtime::LanguageEngine for ThinkingEngine {
        async fn request(
            &self,
            _context: &structured_agent_interpreter_runtime::Context,
            request: &dyn structured_agent_interpreter_runtime::Event,
        ) -> Result<
            (
                ExpressionValue,
                Option<structured_agent_interpreter_runtime::ThinkingEvent>,
            ),
            String,
        > {
            use structured_agent_interpreter_runtime::ThinkingEvent;
            let return_type = request.return_type();
            let value = if return_type.is_boolean() {
                ExpressionValue::boolean(true)
            } else {
                ExpressionValue::string("result".to_string())
            };
            let thinking = ThinkingEvent {
                content: "some thoughts".to_string(),
                thought_signature: None,
            };
            Ok((value, Some(thinking)))
        }
    }

    struct ThinkingEngineRuntime;

    impl RuntimeService for ThinkingEngineRuntime {
        fn get_native_function(&self, _name: &str) -> Option<Arc<dyn ExecutableFunction>> {
            None
        }
        fn get_bytecode_ref(&self, _name: &DefinitionPath) -> Option<BytecodeRef> {
            None
        }
        fn engine(&self) -> &dyn structured_agent_interpreter_runtime::LanguageEngine {
            &ThinkingEngine
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

    #[tokio::test]
    async fn execute_meta_function_with_type_path_produces_metadata() {
        let main_path = DefinitionPath::for_function(DefinitionPath::root(), "main");
        let type_path = DefinitionPath::root()
            .with_module("test".to_string())
            .with_type("Task".to_string());

        let main_ref = make_bytecode_ref(
            vec![
                Instruction::MetaFunction {
                    function_name: type_path,
                    dest: Slot(1),
                },
                Instruction::Ret { var: Slot(1) },
            ],
            2,
            vec![],
        );

        let mut functions = HashMap::new();
        functions.insert(main_path.clone(), main_ref);
        let runtime = Arc::new(TestRuntime::new(functions));
        let context = make_context_with_runtime(runtime.clone());

        let caller_instructions = vec![
            Instruction::CallBytecode {
                function_name: main_path,
                params: vec![],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ];

        let vm = VM::new(runtime);
        let (_, result) = vm
            .execute(&caller_instructions, context, vec![None])
            .await
            .unwrap();
        let (name, _) = result.value.as_metadata().unwrap();
        assert_eq!(name, "test::Task");
    }

    #[tokio::test]
    async fn generate_publishes_thinking_event_when_engine_returns_thinking() {
        use structured_agent_runtime::{AgentHandle, AgentMessageContent};
        let handle = AgentHandle::detached();
        let mut subscriber = handle.subscribe();
        let runtime = Arc::new(ThinkingEngineRuntime);
        let context = structured_agent_interpreter_runtime::Context::with_runtime_and_handle(
            runtime.clone(),
            handle,
        );

        let instructions = vec![
            Instruction::LlmGenerate {
                dest: Slot(0),
                return_type: Type::string(),
            },
            Instruction::Ret { var: Slot(0) },
        ];
        let vm = VM::new(runtime);
        let frame = vec![None];
        vm.execute(&instructions, context, frame).await.unwrap();

        let msgs: Vec<_> = std::iter::from_fn(|| subscriber.try_recv().ok()).collect();
        assert!(
            msgs.iter()
                .any(|m| matches!(m.content, AgentMessageContent::Thinking { .. })),
            "expected a Thinking event to be published"
        );
    }
}
