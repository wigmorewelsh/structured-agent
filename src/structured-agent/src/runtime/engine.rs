use crate::cli::config::{Config, EngineType, McpServerConfig, ProgramSource};
use crate::compiler::{CompilationUnit, Compiler, CompilerTrait};
use crate::expressions::{FunctionExpr, NativeFunctionExpr};
use crate::functions::{InputFunction, PrintFunction};
use crate::gemini::{GeminiConfig, GeminiEngine};
use crate::mcp::McpClient;
use crate::runtime::{Context, ExpressionValue, NativeFunctionProvider};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, FunctionProvider, LanguageEngine,
    NativeFunction,
};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{debug, error};

pub struct Runtime {
    function_registry: HashMap<String, Rc<dyn ExecutableFunction>>,
    external_function_registry: HashMap<String, ExternalFunctionDefinition>,
    language_engine: Rc<dyn LanguageEngine>,
    compiler: Rc<dyn CompilerTrait>,
    providers: Vec<Rc<dyn FunctionProvider>>,
    compiled_program: CompilationUnit,
}

pub struct RuntimeBuilder {
    providers: Vec<Rc<dyn FunctionProvider>>,
    native_provider: NativeFunctionProvider,
    language_engine: Option<Rc<dyn LanguageEngine>>,
    compiler: Option<Rc<dyn CompilerTrait>>,
    program_source: CompilationUnit,
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

impl RuntimeBuilder {
    pub fn new(program: CompilationUnit) -> Self {
        Self {
            providers: Vec::new(),
            native_provider: NativeFunctionProvider::new(),
            language_engine: None,
            compiler: None,
            program_source: program,
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

    pub fn with_provider(mut self, provider: Rc<dyn FunctionProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn with_native_function(mut self, native_function: Arc<dyn NativeFunction>) -> Self {
        self.native_provider.add_function(native_function);
        self
    }

    pub fn with_mcp_client(mut self, client: McpClient) -> Self {
        self.providers.push(Rc::new(client));
        self
    }

    pub fn with_program(mut self, program: CompilationUnit) -> Self {
        self.program_source = program;
        self
    }

    pub fn with_mcp_clients(mut self, clients: Vec<McpClient>) -> Self {
        for client in clients {
            self.providers.push(Rc::new(client));
        }
        self
    }

    pub async fn with_mcp_server_configs(
        mut self,
        configs: &[McpServerConfig],
    ) -> Result<Self, String> {
        for config in configs {
            match McpClient::new_stdio(&config.command, config.args.clone()).await {
                Ok(client) => {
                    self.providers.push(Rc::new(client));
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to connect to MCP server '{}': {}",
                        config.command, e
                    ));
                }
            }
        }
        Ok(self)
    }

    pub fn with_native_provider(mut self, provider: NativeFunctionProvider) -> Self {
        self.providers.push(Rc::new(provider));
        self
    }

    pub async fn from_config(mut self, config: &Config) -> Result<Runtime, String> {
        self = self.with_mcp_server_configs(&config.mcp_servers).await?;

        let engine: Rc<dyn LanguageEngine> = match &config.engine {
            EngineType::Print => Rc::new(crate::types::PrintEngine {}),
            EngineType::Gemini(api_key) => {
                let gemini_config = if let Some(key) = api_key {
                    GeminiConfig::default().with_api_key_auth(key.clone())
                } else {
                    GeminiConfig::from_env().map_err(|e| {
                        format!("Failed to load Gemini config from environment: {}", e)
                    })?
                };

                match GeminiEngine::new(gemini_config).await {
                    Ok(gemini) => Rc::new(gemini),
                    Err(e) => {
                        return Err(format!("Failed to initialize Gemini engine: {}", e));
                    }
                }
            }
        };

        self = self.with_engine(engine);

        if config.with_default_functions {
            self = self
                .with_native_function(Arc::new(InputFunction::new()))
                .with_native_function(Arc::new(PrintFunction::new()));
        }

        Ok(self.build())
    }

    pub fn build(self) -> Runtime {
        let native_provider_rc = Rc::new(self.native_provider);
        let mut providers = self.providers;
        providers.push(native_provider_rc.clone());

        let mut function_registry = HashMap::new();

        for (name, native_fn) in &native_provider_rc.native_functions {
            let expr = NativeFunctionExpr::new(native_fn.clone());
            function_registry.insert(name.clone(), Rc::new(expr) as Rc<dyn ExecutableFunction>);
        }

        Runtime {
            function_registry,
            external_function_registry: HashMap::new(),
            language_engine: self
                .language_engine
                .unwrap_or_else(|| Rc::new(crate::types::PrintEngine {})),
            compiler: self.compiler.unwrap_or_else(|| Rc::new(Compiler::new())),
            providers,
            compiled_program: self.program_source,
        }
    }
}

impl Runtime {
    pub fn builder(program: CompilationUnit) -> RuntimeBuilder {
        RuntimeBuilder::new(program)
    }

    pub fn register_function(&mut self, function: FunctionExpr) {
        self.function_registry
            .insert(function.name.clone(), Rc::new(function));
    }

    pub fn register_expression(&mut self, name: String, expression: Rc<dyn ExecutableFunction>) {
        self.function_registry.insert(name, expression);
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

    pub fn check(&self) -> Result<(), RuntimeError> {
        debug!("Starting program check");
        match self.compiler.compile_program(&self.compiled_program) {
            Ok(_) => {
                debug!("Program check completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Program check failed: {}", e);
                Err(RuntimeError::ExecutionError(e))
            }
        }
    }

    pub async fn run(&self) -> Result<ExpressionValue, RuntimeError> {
        debug!("Starting program execution");

        let compiled_program = match self.compiler.compile_program(&self.compiled_program) {
            Ok(program) => {
                debug!("Program compiled successfully");
                debug!(
                    "Functions: {:?}",
                    program.functions().keys().collect::<Vec<_>>()
                );
                debug!(
                    "External functions: {:?}",
                    program.external_functions().keys().collect::<Vec<_>>()
                );
                program
            }
            Err(e) => {
                error!("Compilation failed: {}", e);
                return Err(RuntimeError::ExecutionError(e));
            }
        };

        let mut runtime = Runtime {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            providers: self.providers.clone(),
            compiled_program: self.compiled_program.clone(),
        };

        for function in compiled_program.functions().values() {
            debug!("Registering function: {}", function.name);
            runtime.register_function(function.clone());
        }
        for external_function in compiled_program.external_functions().values() {
            debug!("Registering external function: {}", external_function.name);
            runtime.register_external_function(external_function.clone());
        }

        if let Err(e) = runtime.map_providers_to_functions().await {
            error!("Failed to map providers to functions: {:?}", e);
            return Err(e);
        }

        if let Some(main_function) = compiled_program.main_function() {
            debug!("Executing main function");
            match runtime.run_expression(main_function).await {
                Ok(result) => {
                    debug!("Program execution completed successfully");
                    debug!("Result type: {}", result.type_name());
                    Ok(result)
                }
                Err(e) => {
                    error!("Runtime execution failed: {:?}", e);
                    Err(e)
                }
            }
        } else {
            error!("No main function found in program");
            Err(RuntimeError::FunctionNotFound("main".to_string()))
        }
    }

    pub async fn run_expression(
        &self,
        program: &dyn crate::types::Expression,
    ) -> Result<ExpressionValue, RuntimeError> {
        debug!("Running expression");
        let initial_context = Arc::new(Context::with_runtime(Rc::new(self.create_runtime_ref())));
        match program.evaluate(initial_context).await {
            Ok(result) => {
                debug!("Expression evaluated successfully");
                Ok(result.value)
            }
            Err(e) => {
                error!("Expression evaluation failed: {}", e);
                Err(RuntimeError::ExecutionError(e))
            }
        }
    }

    fn create_runtime_ref(&self) -> Runtime {
        Runtime {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            providers: self.providers.clone(),
            compiled_program: self.compiled_program.clone(),
        }
    }

    fn signatures_match(
        provider_def: &ExternalFunctionDefinition,
        definition: &ExternalFunctionDefinition,
    ) -> bool {
        provider_def.parameters.len() == definition.parameters.len()
            && provider_def.return_type == definition.return_type
            && provider_def
                .parameters
                .iter()
                .zip(&definition.parameters)
                .all(|(provider_param, extern_param)| {
                    provider_param.name == extern_param.name
                        && provider_param.param_type == extern_param.param_type
                })
    }

    fn find_matching_provider<'a>(
        matches: &'a [(ExternalFunctionDefinition, Rc<dyn FunctionProvider>)],
        definition: &ExternalFunctionDefinition,
        name: &str,
    ) -> Result<&'a Rc<dyn FunctionProvider>, RuntimeError> {
        matches
            .iter()
            .find(|(provider_def, _)| Self::signatures_match(provider_def, definition))
            .map(|(_, provider)| provider)
            .ok_or_else(|| {
                let expected_params = definition
                    .parameters
                    .iter()
                    .map(|p| format!("{}: {:?}", p.name, p.param_type))
                    .collect::<Vec<_>>()
                    .join(", ");

                let available_sigs = matches
                    .iter()
                    .map(|(provider_def, _)| {
                        let params = provider_def
                            .parameters
                            .iter()
                            .map(|p| format!("{}: {:?}", p.name, p.param_type))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("  - fn {}({}) -> {:?}", name, params, provider_def.return_type)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                RuntimeError::ExecutionError(format!(
                    "No matching provider found for extern function '{}'.\n\nExpected signature:\n  fn {}({}) -> {:?}\n\nAvailable signatures from providers:\n{}",
                    name, name, expected_params, definition.return_type, available_sigs
                ))
            })
    }

    async fn map_providers_to_functions(&mut self) -> Result<(), RuntimeError> {
        let mut provider_functions = HashMap::new();

        for provider in &self.providers {
            let available_functions = provider.list_functions().await?;

            for func_def in available_functions {
                provider_functions
                    .entry(func_def.name.clone())
                    .or_insert_with(Vec::new)
                    .push((func_def, provider.clone()));
            }
        }

        let mut functions_to_register = Vec::new();

        for (name, definition) in &self.external_function_registry {
            let matches = provider_functions.get(name).ok_or_else(|| {
                RuntimeError::ExecutionError(format!(
                    "No provider found for extern function '{}'",
                    name
                ))
            })?;

            let provider = Self::find_matching_provider(matches, definition, name)?;
            let expr = provider.create_expression(definition).await?;
            functions_to_register.push((name.clone(), expr));
        }

        for (name, expr) in functions_to_register {
            self.register_expression(name, expr);
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn providers_count(&self) -> usize {
        self.providers.len()
    }

    #[cfg(test)]
    pub async fn test_map_providers_to_functions(&mut self) -> Result<(), RuntimeError> {
        self.map_providers_to_functions().await
    }
}

impl Clone for Runtime {
    fn clone(&self) -> Self {
        Self {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            providers: self.providers.clone(),
            compiled_program: self.compiled_program.clone(),
        }
    }
}

pub fn load_program(source: &ProgramSource) -> Result<CompilationUnit, std::io::Error> {
    match source {
        ProgramSource::Inline(code) => Ok(CompilationUnit::from_string(code.clone())),
        ProgramSource::File(path) => {
            let content = std::fs::read_to_string(path)?;
            Ok(CompilationUnit::from_file(path.clone(), content))
        }
    }
}
