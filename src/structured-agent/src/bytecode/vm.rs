use super::{CompiledFunction, Instruction};
use crate::runtime::{Context, ExpressionParameter, ExpressionResult, ExpressionValue, Runtime};
use std::rc::Rc;
use std::sync::Arc;

pub struct VMState {
    pc: usize,
    current_context: Arc<Context>,
}

pub struct VM {
    runtime: Rc<Runtime>,
}

impl VM {
    pub fn new(runtime: Rc<Runtime>) -> Self {
        Self { runtime }
    }

    pub async fn execute(
        &self,
        function: &CompiledFunction,
        context: Arc<Context>,
    ) -> Result<ExpressionResult, String> {
        let mut state = VMState {
            pc: 0,
            current_context: context.clone(),
        };

        loop {
            if state.pc >= function.instructions.len() {
                return Err("PC out of bounds".to_string());
            }

            let instruction = &function.instructions[state.pc];

            match instruction {
                Instruction::Nop => state.pc += 1,
                Instruction::Drop { name } => self.execute_drop(&mut state, name),
                Instruction::LdcStr { dest, value } => {
                    self.execute_ldc_str(&mut state, dest, value)
                }
                Instruction::LdcBool { dest, value } => {
                    self.execute_ldc_bool(&mut state, dest, *value)
                }
                Instruction::LdcUnit { dest } => self.execute_ldc_unit(&mut state, dest),
                Instruction::Mov { dest, src } => self.execute_mov(&mut state, dest, src)?,
                Instruction::Decl { name } => self.execute_decl(&mut state, name),
                Instruction::Br { offset } => state.pc = *offset as usize,
                Instruction::BrFalse { var, offset } => {
                    Self::branch_if_bool(&mut state, var, *offset, false)?
                }
                Instruction::BrTrue { var, offset } => {
                    Self::branch_if_bool(&mut state, var, *offset, true)?
                }
                Instruction::Switch { var, offsets } => {
                    self.execute_switch(&mut state, var, offsets)?
                }
                Instruction::Ret { var } => return self.execute_ret(&state, var),
                Instruction::Yield => return Err("Yield not yet implemented".to_string()),
                Instruction::Call {
                    function_name,
                    params,
                    dest,
                } => {
                    self.execute_call(&mut state, function_name, params, dest)
                        .await?
                }
                Instruction::CtxEvent { var } => self.execute_ctx_event(&mut state, var)?,
                Instruction::CtxChild { is_scope_boundary } => {
                    self.execute_ctx_child(&mut state, *is_scope_boundary)
                }
                Instruction::CtxRestore => self.execute_ctx_restore(&mut state)?,
                Instruction::MetaFunction {
                    function_name,
                    dest,
                } => self.execute_meta_function(&mut state, function_name, dest)?,
                Instruction::ListNew {
                    dest,
                    element_type: _,
                } => self.execute_list_new(&mut state, dest),
                Instruction::ListAdd { dest: _, src: _ } => Self::advance_pc(&mut state),
                Instruction::ListFinish { dest: _ } => Self::advance_pc(&mut state),
                Instruction::LlmPlaceholder {
                    dest,
                    param_name,
                    param_type,
                } => {
                    self.execute_llm_placeholder(&mut state, dest, param_name, param_type)
                        .await?
                }
                Instruction::LlmSelect {
                    metadata_vars,
                    dest,
                } => {
                    self.execute_llm_select(&mut state, metadata_vars, dest)
                        .await?
                }
                Instruction::LlmGenerate { dest, return_type } => {
                    self.execute_llm_generate(&mut state, dest, return_type)
                        .await?
                }
            }
        }
    }

    // ===== Literal Instructions =====

    fn execute_ldc_str(&self, state: &mut VMState, dest: &str, value: &str) {
        Self::write_variable(
            state,
            dest,
            ExpressionResult::new(ExpressionValue::String(value.to_string())),
        );
        Self::advance_pc(state);
    }

    fn execute_ldc_bool(&self, state: &mut VMState, dest: &str, value: bool) {
        Self::write_variable(
            state,
            dest,
            ExpressionResult::new(ExpressionValue::Boolean(value)),
        );
        Self::advance_pc(state);
    }

    fn execute_ldc_unit(&self, state: &mut VMState, dest: &str) {
        Self::write_variable(state, dest, ExpressionResult::new(ExpressionValue::Unit));
        Self::advance_pc(state);
    }

    // ===== Variable Instructions =====

    fn execute_mov(&self, state: &mut VMState, dest: &str, src: &str) -> Result<(), String> {
        let value = Self::read_variable(state, src)?;
        state
            .current_context
            .assign_variable(dest.to_string(), value)?;
        Self::advance_pc(state);
        Ok(())
    }

    fn execute_decl(&self, state: &mut VMState, name: &str) {
        Self::write_variable(state, name, ExpressionResult::new(ExpressionValue::Unit));
        Self::advance_pc(state);
    }

    fn execute_drop(&self, state: &mut VMState, name: &str) {
        state.current_context.remove_variable(name);
        Self::advance_pc(state);
    }

    // ===== Control Flow Instructions =====

    fn execute_switch(
        &self,
        state: &mut VMState,
        var: &str,
        offsets: &[i32],
    ) -> Result<(), String> {
        let value = Self::read_variable(state, var)?;

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
            state.pc = offsets[index] as usize;
        } else {
            return Err(format!("Switch index {} out of range", index));
        }
        Ok(())
    }

    fn execute_ret(&self, state: &VMState, var: &str) -> Result<ExpressionResult, String> {
        let result = Self::read_variable(state, var)?;
        state.current_context.set_return_value(result.clone());
        Ok(result)
    }

    // ===== Function Call Instructions =====

    async fn execute_call(
        &self,
        state: &mut VMState,
        function_name: &str,
        params: &[String],
        dest: &str,
    ) -> Result<(), String> {
        let child_context = Arc::new(Context::create_child(
            state.current_context.clone(),
            true,
            self.runtime.clone(),
        ));

        child_context.add_event(
            ExpressionValue::String(format!("## {}", function_name)),
            None,
            None,
        );

        let func = self
            .runtime
            .get_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let function_params = func.parameters();
        if params.len() != function_params.len() {
            return Err(format!(
                "Function {} expects {} arguments, got {}",
                function_name,
                function_params.len(),
                params.len()
            ));
        }

        let mut evaluated_parameters = Vec::new();

        for (i, var_name) in params.iter().enumerate() {
            let value = Self::read_variable(state, var_name)?;
            let actual_param_name = &function_params[i].name;
            child_context.declare_variable(actual_param_name.clone(), value.clone());
            evaluated_parameters.push(ExpressionParameter::new(
                actual_param_name.clone(),
                value.value.clone(),
            ));
        }

        let result = func.evaluate(child_context.clone()).await?;

        let result_with_metadata = ExpressionResult {
            name: Some(function_name.to_string()),
            params: Some(evaluated_parameters),
            value: result.value,
        };

        Self::write_variable(state, dest, result_with_metadata);
        Self::advance_pc(state);
        Ok(())
    }

    // ===== Context Instructions =====

    fn execute_ctx_event(&self, state: &mut VMState, var: &str) -> Result<(), String> {
        let expr_result = Self::read_variable(state, var)?;

        state.current_context.add_event(
            expr_result.value.clone(),
            expr_result.name.clone(),
            expr_result.params.clone(),
        );
        Self::advance_pc(state);
        Ok(())
    }

    fn execute_ctx_child(&self, state: &mut VMState, is_scope_boundary: bool) {
        let child = Arc::new(Context::create_child(
            state.current_context.clone(),
            is_scope_boundary,
            self.runtime.clone(),
        ));
        state.current_context = child;
        Self::advance_pc(state);
    }

    fn execute_ctx_restore(&self, state: &mut VMState) -> Result<(), String> {
        state.current_context = state
            .current_context
            .parent_context()
            .ok_or("No parent context to restore")?;
        Self::advance_pc(state);
        Ok(())
    }

    // ===== Metadata Instructions =====

    fn execute_meta_function(
        &self,
        state: &mut VMState,
        function_name: &str,
        dest: &str,
    ) -> Result<(), String> {
        let func = self
            .runtime
            .get_function(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        let metadata = ExpressionValue::Metadata {
            name: function_name.to_string(),
            documentation: func.documentation().map(|s| s.to_string()),
        };

        Self::write_variable(state, dest, ExpressionResult::new(metadata));
        Self::advance_pc(state);
        Ok(())
    }

    // ===== List Instructions =====

    fn execute_list_new(&self, state: &mut VMState, dest: &str) {
        Self::write_variable(state, dest, ExpressionResult::new(ExpressionValue::Unit));
        Self::advance_pc(state);
    }

    // ===== LLM Instructions =====

    async fn execute_llm_placeholder(
        &self,
        state: &mut VMState,
        dest: &str,
        param_name: &str,
        param_type: &str,
    ) -> Result<(), String> {
        let param_type_obj = parse_type(param_type)?;
        let value = state
            .current_context
            .runtime()
            .engine()
            .fill_parameter(&state.current_context, param_name, &param_type_obj)
            .await?;

        Self::write_variable(state, dest, ExpressionResult::new(value));
        Self::advance_pc(state);
        Ok(())
    }

    async fn execute_llm_select(
        &self,
        state: &mut VMState,
        metadata_vars: &[String],
        dest: &str,
    ) -> Result<(), String> {
        let mut metadata_values = Vec::new();

        for var_name in metadata_vars {
            let value = Self::read_variable(state, var_name)?;
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
            .current_context
            .runtime()
            .engine()
            .select(&state.current_context, &metadata_values)
            .await?;

        let result = ExpressionResult::new(ExpressionValue::String(selected_index.to_string()));

        Self::write_variable(state, dest, result);
        Self::advance_pc(state);
        Ok(())
    }

    async fn execute_llm_generate(
        &self,
        state: &mut VMState,
        dest: &str,
        return_type: &str,
    ) -> Result<(), String> {
        let return_type_obj = parse_type(return_type)?;
        let value = state
            .current_context
            .runtime()
            .engine()
            .typed(&state.current_context, &return_type_obj)
            .await?;

        Self::write_variable(state, dest, ExpressionResult::new(value));
        Self::advance_pc(state);
        Ok(())
    }

    // ===== Helper Methods =====

    fn advance_pc(state: &mut VMState) {
        state.pc += 1;
    }

    fn read_variable(state: &VMState, name: &str) -> Result<ExpressionResult, String> {
        state
            .current_context
            .get_variable(name)
            .ok_or_else(|| format!("Variable not found: {}", name))
    }

    fn write_variable(state: &VMState, name: &str, value: ExpressionResult) {
        state
            .current_context
            .declare_variable(name.to_string(), value);
    }

    fn branch_if_bool(
        state: &mut VMState,
        var: &str,
        offset: i32,
        expected: bool,
    ) -> Result<(), String> {
        let value = Self::read_variable(state, var)?;

        match &value.value {
            ExpressionValue::Boolean(b) if *b == expected => {
                state.pc = offset as usize;
            }
            ExpressionValue::Boolean(_) => {
                Self::advance_pc(state);
            }
            _ => {
                return Err(format!(
                    "Expected boolean for branch, got {:?}",
                    value.value
                ));
            }
        }
        Ok(())
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
