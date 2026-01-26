use crate::compiler::{CompilationUnit, Compiler, CompilerTrait};
use crate::expressions::{ExternalFunctionExpr, FunctionExpr, NativeFunctionExpr};
use crate::mcp::McpClient;
use crate::runtime::{Context, ExprResult};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, LanguageEngine, NativeFunction,
};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub struct Runtime {
    function_registry: HashMap<String, Rc<dyn ExecutableFunction>>,
    external_function_registry: HashMap<String, ExternalFunctionDefinition>,
    language_engine: Rc<dyn LanguageEngine>,
    compiler: Rc<dyn CompilerTrait>,
    mcp_clients: Vec<Rc<McpClient>>,
}

pub struct RuntimeBuilder {
    function_registry: HashMap<String, Arc<dyn NativeFunction>>,
    external_function_registry: HashMap<String, ExternalFunctionDefinition>,
    language_engine: Option<Rc<dyn LanguageEngine>>,
    compiler: Option<Rc<dyn CompilerTrait>>,
    mcp_clients: Vec<McpClient>,
}

#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    FunctionNotFound(String),
    InvalidArguments(String),
    ExecutionError(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::FunctionNotFound(name) => write!(f, "Function not found: {}", name),
            RuntimeError::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            RuntimeError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            function_registry: HashMap::new(),
            external_function_registry: HashMap::new(),
            language_engine: None,
            compiler: None,
            mcp_clients: Vec::new(),
        }
    }

    pub fn with_engine(mut self, engine: Rc<dyn LanguageEngine>) -> Self {
        self.language_engine = Some(engine);
        self
    }

    pub fn with_compiler(mut self, compiler: Rc<dyn CompilerTrait>) -> Self {
        self.compiler = Some(compiler);
        self
    }

    pub fn with_mcp_client(mut self, client: McpClient) -> Self {
        self.mcp_clients.push(client);
        self
    }

    pub fn with_mcp_clients(mut self, clients: Vec<McpClient>) -> Self {
        self.mcp_clients.extend(clients);
        self
    }

    pub fn with_native_function(mut self, native_function: Arc<dyn NativeFunction>) -> Self {
        let name = native_function.name().to_string();
        self.function_registry.insert(name, native_function);
        self
    }

    pub fn build(self) -> Runtime {
        let mut function_registry = HashMap::new();

        for (name, native_function) in self.function_registry {
            let expr = NativeFunctionExpr::new(native_function);
            function_registry.insert(name, Rc::new(expr) as Rc<dyn ExecutableFunction>);
        }

        Runtime {
            function_registry,
            external_function_registry: self.external_function_registry,
            language_engine: self
                .language_engine
                .unwrap_or_else(|| Rc::new(crate::types::PrintEngine {})),
            compiler: self.compiler.unwrap_or_else(|| Rc::new(Compiler::new())),
            mcp_clients: self.mcp_clients.into_iter().map(Rc::new).collect(),
        }
    }
}

impl Runtime {
    pub fn new() -> Self {
        RuntimeBuilder::new().build()
    }

    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn register_function(&mut self, function: FunctionExpr) {
        self.function_registry
            .insert(function.name.clone(), Rc::new(function));
    }

    pub fn register_expression(&mut self, name: String, expression: Rc<dyn ExecutableFunction>) {
        self.function_registry.insert(name, expression);
    }

    pub fn register_native_function(&mut self, native_function: Arc<dyn NativeFunction>) {
        let name = native_function.name().to_string();
        let expr = NativeFunctionExpr::new(native_function);
        self.function_registry.insert(name, Rc::new(expr));
    }

    pub fn get_function(&self, name: &str) -> Option<&dyn ExecutableFunction> {
        self.function_registry.get(name).map(|expr| expr.as_ref())
    }

    pub fn register_external_function(&mut self, function: ExternalFunctionDefinition) {
        self.external_function_registry
            .insert(function.name.clone(), function);
    }

    pub fn get_external_function(&self, name: &str) -> Option<&ExternalFunctionDefinition> {
        self.external_function_registry.get(name)
    }

    pub fn list_functions(&self) -> Vec<&str> {
        self.function_registry.keys().map(|s| s.as_str()).collect()
    }

    pub fn engine(&self) -> &dyn LanguageEngine {
        self.language_engine.as_ref()
    }

    pub fn compiler(&self) -> &dyn CompilerTrait {
        self.compiler.as_ref()
    }

    pub async fn run(&self, program_source: &str) -> Result<ExprResult, RuntimeError> {
        let program = CompilationUnit::from_string(program_source.to_string());
        let compiled_program = self
            .compiler
            .compile_program(&program)
            .map_err(RuntimeError::ExecutionError)?;

        let mut runtime = Runtime {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            mcp_clients: self.mcp_clients.clone(),
        };

        for function in compiled_program.functions().values() {
            runtime.register_function(function.clone());
        }
        for external_function in compiled_program.external_functions().values() {
            runtime.register_external_function(external_function.clone());
        }

        runtime.map_mcp_tools_to_external_functions().await?;

        if let Some(main_function) = compiled_program.main_function() {
            runtime.run_expression(main_function).await
        } else {
            Err(RuntimeError::FunctionNotFound("main".to_string()))
        }
    }

    pub async fn run_expression(
        &self,
        program: &dyn crate::types::Expression,
    ) -> Result<ExprResult, RuntimeError> {
        let initial_context = Arc::new(Context::with_runtime(Rc::new(self.create_runtime_ref())));
        program
            .evaluate(initial_context)
            .await
            .map_err(RuntimeError::ExecutionError)
    }

    fn create_runtime_ref(&self) -> Runtime {
        Runtime {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            mcp_clients: self.mcp_clients.clone(),
        }
    }

    async fn map_mcp_tools_to_external_functions(&mut self) -> Result<(), RuntimeError> {
        let mut functions_to_register = Vec::new();

        for client in &self.mcp_clients {
            let tools = client.list_tools().await.map_err(|e| {
                RuntimeError::ExecutionError(format!("Failed to list MCP tools: {}", e))
            })?;

            for tool in tools {
                if let Some(external_fn) = self.external_function_registry.get(&tool.name) {
                    let external_function_expr = ExternalFunctionExpr::new(
                        tool.name.clone(),
                        external_fn.parameters.clone(),
                        external_fn.return_type.clone(),
                        client.clone(),
                        external_fn.documentation.clone(),
                    );

                    functions_to_register.push((tool.name.clone(), external_function_expr));
                }
            }
        }

        for (name, expr) in functions_to_register {
            self.register_expression(name, Rc::new(expr));
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn mcp_clients_count(&self) -> usize {
        self.mcp_clients.len()
    }

    #[cfg(test)]
    pub async fn test_map_mcp_tools_to_external_functions(&mut self) -> Result<(), RuntimeError> {
        self.map_mcp_tools_to_external_functions().await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Runtime {
    fn clone(&self) -> Self {
        Self {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            mcp_clients: self.mcp_clients.clone(),
        }
    }
}
