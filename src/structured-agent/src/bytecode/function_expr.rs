use crate::bytecode::{BytecodeRef, VM};
use crate::runtime::{Context, ExpressionResult};
use crate::types::{ExecutableFunction, Function, Parameter, Type};
use async_trait::async_trait;
use std::any::Any;
use structured_agent_runtime::FunctionName;

pub struct BytecodeFunctionExpr {
    name: String,
    parameters: Vec<Parameter>,
    return_type: Type,
    instructions: Vec<crate::bytecode::Instruction>,
    labels: std::collections::HashMap<String, usize>,
    documentation: Option<String>,
}

impl BytecodeFunctionExpr {
    pub fn new(name: FunctionName, body: BytecodeRef) -> Self {
        Self {
            name: name.to_string(),
            parameters: body.parameters,
            return_type: body.return_type,
            instructions: body.instructions,
            labels: body.labels,
            documentation: body.documentation,
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
        mut context: Context,
        args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String> {
        for (i, param) in self.parameters.iter().enumerate() {
            context.declare_variable(param.name.clone(), args[i].clone());
        }

        let vm = VM::new(context.runtime_arc());
        let result = vm.execute(&self.instructions, context).await?;
        let returned_context = result.0;
        let returned_result = result.1;
        Ok((returned_context, returned_result))
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
