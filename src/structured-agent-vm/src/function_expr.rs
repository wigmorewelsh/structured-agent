use crate::vm::VM;
use async_trait::async_trait;
use std::any::Any;
use structured_agent_il::BytecodeRef;
use structured_agent_interpreter_runtime::{
    Context, ExecutableFunction, ExpressionResult, Function, Parameter, Type,
};
use structured_agent_runtime::DefinitionPath;

pub struct BytecodeFunctionExpr {
    name: String,
    parameters: Vec<Parameter>,
    return_type: Type,
    instructions: Vec<structured_agent_il::Instruction>,
    labels: std::collections::HashMap<String, usize>,
    documentation: Option<String>,
    slot_count: usize,
}

impl BytecodeFunctionExpr {
    pub fn new(name: DefinitionPath, body: BytecodeRef) -> Self {
        Self {
            name: name.to_string(),
            parameters: body.parameters,
            return_type: body.return_type,
            instructions: body.instructions,
            labels: body.labels,
            documentation: body.documentation,
            slot_count: body.slot_table.len(),
        }
    }
}

impl std::fmt::Debug for BytecodeFunctionExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BytecodeFunctionExpr")
            .field("name", &self.name)
            .field("parameters", &self.parameters)
            .field("return_type", &self.return_type)
            .field(
                "instructions",
                &format!("[{} instructions]", self.instructions.len()),
            )
            .finish()
    }
}

impl Clone for BytecodeFunctionExpr {
    fn clone(&self) -> Self {
        BytecodeFunctionExpr {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            instructions: self.instructions.clone(),
            labels: self.labels.clone(),
            documentation: self.documentation.clone(),
            slot_count: self.slot_count,
        }
    }
}

#[async_trait]
impl Function for BytecodeFunctionExpr {
    fn name(&self) -> &str {
        &self.name
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn function_return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        context: Context,
        args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String> {
        let mut frame: Vec<Option<ExpressionResult>> = vec![None; self.slot_count];
        for (i, arg) in args.iter().enumerate() {
            frame[i + 1] = Some(arg.clone());
        }

        let vm = VM::new(context.runtime_arc());
        let result = vm.execute(&self.instructions, context, frame).await?;
        Ok((result.0, result.1))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Function> {
        Box::new(self.clone())
    }

    fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }
}

#[async_trait]
impl ExecutableFunction for BytecodeFunctionExpr {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}
