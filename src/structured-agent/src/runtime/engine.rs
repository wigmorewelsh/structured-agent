use crate::bytecode::BytecodeFunctionExpr;
use crate::cli::config::{Config, EngineType, McpServerConfig, ProgramSource};
use crate::compiler::{CompilationUnit, CompiledProgram, Compiler};
use crate::functions::{
    HeadFunction, InputFunction, IsSomeFunction, PrintFunction, SomeValueFunction, TailFunction,
    acp_shim,
};
use crate::gemini::{GeminiConfig, GeminiEngine};
use crate::mcp::McpClient;
use crate::runtime::{Context, ExpressionValue, NativeFunctionProvider};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, Function, FunctionProvider, LanguageEngine,
    NativeFunction,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error};

pub struct Runtime {
    function_registry: HashMap<String, Arc<dyn ExecutableFunction>>,
    external_function_registry: HashMap<String, ExternalFunctionDefinition>,
    struct_registry: HashMap<String, Vec<(String, crate::types::Type)>>,
    language_engine: Arc<dyn LanguageEngine>,
    compiler: Arc<Compiler>,
    providers: Vec<Arc<dyn FunctionProvider>>,
    program_source: ProgramSource,
    vtables: HashMap<String, HashMap<String, String>>,
}

pub struct RuntimeBuilder {
    providers: Vec<Arc<dyn FunctionProvider>>,
    native_provider: NativeFunctionProvider,
    language_engine: Option<Arc<dyn LanguageEngine>>,
    compiler: Option<Arc<Compiler>>,
    program_source: ProgramSource,
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
    pub fn new(source: ProgramSource) -> Self {
        Self {
            providers: Vec::new(),
            native_provider: NativeFunctionProvider::new(),
            language_engine: None,
            compiler: None,
            program_source: source,
        }
    }

    pub fn with_language_engine(mut self, engine: Arc<dyn LanguageEngine>) -> Self {
        self.language_engine = Some(engine);
        self
    }

    pub fn with_compiler(mut self, compiler: Arc<Compiler>) -> Self {
        self.compiler = Some(compiler);
        self
    }

    pub fn with_provider(mut self, provider: Arc<dyn FunctionProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn with_native_function<F: NativeFunction + 'static>(
        mut self,
        native_function: Arc<F>,
    ) -> Self {
        self.native_provider.add_function(native_function);
        self
    }

    pub fn with_mcp_client(mut self, client: McpClient) -> Self {
        self.providers.push(Arc::new(client));
        self
    }

    pub fn with_mcp_clients(mut self, clients: Vec<McpClient>) -> Self {
        for client in clients {
            self.providers.push(Arc::new(client));
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
                    self.providers.push(Arc::new(client));
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
        self.providers.push(Arc::new(provider));
        self
    }

    pub async fn with_config(mut self, config: &Config) -> Result<Runtime, String> {
        self = self.with_mcp_server_configs(&config.mcp_servers).await?;

        let engine: Arc<dyn LanguageEngine> = match &config.engine {
            EngineType::Print => Arc::new(crate::types::PrintEngine {}),
            EngineType::Gemini { api_key, model } => {
                let gemini_config = if let Some(key) = api_key {
                    GeminiConfig::default().with_api_key_auth(key.clone())
                } else {
                    GeminiConfig::from_env().map_err(|e| {
                        format!("Failed to load Gemini config from environment: {}", e)
                    })?
                };

                let mut gemini = match GeminiEngine::new(gemini_config).await {
                    Ok(gemini) => gemini,
                    Err(e) => {
                        return Err(format!("Failed to initialize Gemini engine: {}", e));
                    }
                };

                if let Some(model_name) = model {
                    let model_enum = match model_name.as_str() {
                        "gemini-2.5-pro" => crate::gemini::types::ModelName::Gemini25Pro,
                        "gemini-2.5-flash" => crate::gemini::types::ModelName::Gemini25Flash,
                        "gemini-2.5-flash-lite" => {
                            crate::gemini::types::ModelName::Gemini25FlashLite
                        }
                        "gemini-3-flash-preview" => {
                            crate::gemini::types::ModelName::Gemini3FlashPreview
                        }
                        "gemini-3-pro-preview" => {
                            crate::gemini::types::ModelName::Gemini3ProPreview
                        }
                        custom => crate::gemini::types::ModelName::Custom(custom.to_string()),
                    };
                    gemini = gemini.with_model(model_enum);
                }

                Arc::new(gemini)
            }
        };

        self = self.with_language_engine(engine);

        if config.with_default_functions {
            self = self
                .with_native_function(Arc::new(InputFunction::new()))
                .with_native_function(Arc::new(PrintFunction::new()));
        }

        if config.with_unstable_functions {
            self = self
                .with_native_function(Arc::new(HeadFunction::new()))
                .with_native_function(Arc::new(TailFunction::new()))
                .with_native_function(Arc::new(IsSomeFunction::for_string()))
                .with_native_function(Arc::new(SomeValueFunction::for_string()))
                .with_native_function(Arc::new(IsSomeFunction::for_list()))
                .with_native_function(Arc::new(SomeValueFunction::for_list()));
        }

        if config.with_acp_functions {
            self = self
                .with_native_function(Arc::new(acp_shim::ReceiveFunction::new()))
                .with_native_function(Arc::new(acp_shim::TryReceiveFunction::new()));
        }

        Ok(self.build())
    }

    pub fn build(self) -> Runtime {
        let native_provider_rc = Arc::new(self.native_provider);
        let mut providers = self.providers;
        providers.push(native_provider_rc.clone());

        let function_registry = native_provider_rc.native_functions.clone();

        Runtime {
            function_registry,
            external_function_registry: HashMap::new(),
            struct_registry: HashMap::new(),
            language_engine: self
                .language_engine
                .unwrap_or_else(|| Arc::new(crate::types::PrintEngine {})),
            compiler: self.compiler.unwrap_or_else(|| Arc::new(Compiler::new())),
            providers,
            program_source: self.program_source,
            vtables: HashMap::new(),
        }
    }
}

impl Runtime {
    pub fn builder(source: ProgramSource) -> RuntimeBuilder {
        RuntimeBuilder::new(source)
    }

    pub fn register_function(&mut self, function: Box<dyn ExecutableFunction>) {
        let name = Function::name(function.as_ref()).to_string();
        self.function_registry.insert(name, Arc::from(function));
    }

    pub fn register_expression(&mut self, name: String, expression: Arc<dyn ExecutableFunction>) {
        self.function_registry.insert(name, expression);
    }

    pub fn get_function(&self, name: &str) -> Option<&dyn ExecutableFunction> {
        self.function_registry.get(name).map(|arc| arc.as_ref())
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

    pub fn compiler(&self) -> &Compiler {
        &self.compiler
    }

    pub fn check(&self) -> Result<(), RuntimeError> {
        debug!("Starting program check");
        self.compile()
            .map(|_| ())
            .map_err(RuntimeError::ExecutionError)
    }

    pub async fn run(&self) -> Result<ExpressionValue, RuntimeError> {
        debug!("Starting program execution");

        let compiled_program = self.compile().map_err(RuntimeError::ExecutionError)?;

        let mut runtime = self.create_runtime_ref();
        runtime.vtables = compiled_program.vtables().clone();

        for (name, fields) in compiled_program.struct_definitions() {
            runtime.register_struct(name.clone(), fields.clone());
        }

        for (name, function) in compiled_program.functions() {
            debug!("Registering function: {}", name);
            runtime.function_registry.insert(
                name.clone(),
                Arc::new(BytecodeFunctionExpr::new(function.clone())),
            );
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
            let main_expr = BytecodeFunctionExpr::new(main_function.clone());
            match runtime.run_expression(&main_expr).await {
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
        program: &dyn crate::types::Function,
    ) -> Result<ExpressionValue, RuntimeError> {
        debug!("Running expression");
        let initial_context = Context::with_runtime(Arc::new(self.create_runtime_ref()));
        match program.execute(initial_context, vec![]).await {
            Ok((_context, result)) => {
                debug!("Expression evaluated successfully");
                Ok(result.value)
            }
            Err(e) => {
                error!("Expression evaluation failed: {}", e);
                Err(RuntimeError::ExecutionError(e))
            }
        }
    }

    pub fn get_struct(&self, name: &str) -> Option<&Vec<(String, crate::types::Type)>> {
        self.struct_registry.get(name)
    }

    pub fn register_struct(&mut self, name: String, fields: Vec<(String, crate::types::Type)>) {
        self.struct_registry.insert(name, fields);
    }

    fn compile(&self) -> Result<CompiledProgram, String> {
        match &self.program_source {
            ProgramSource::Inline(source) => {
                let unit = CompilationUnit::from_string(source.clone());
                self.compiler.compile_source(&unit)
            }
            ProgramSource::File(path) => self.compiler.compile_file(path),
        }
    }

    fn create_runtime_ref(&self) -> Runtime {
        Runtime {
            function_registry: self.function_registry.clone(),
            external_function_registry: self.external_function_registry.clone(),
            struct_registry: self.struct_registry.clone(),
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            providers: self.providers.clone(),
            program_source: self.program_source.clone(),
            vtables: self.vtables.clone(),
        }
    }

    pub fn vtables(&self) -> &HashMap<String, HashMap<String, String>> {
        &self.vtables
    }

    fn signatures_match(
        provider_def: &ExternalFunctionDefinition,
        definition: &ExternalFunctionDefinition,
    ) -> bool {
        if provider_def.parameters.len() != definition.parameters.len() {
            return false;
        }

        if provider_def.return_type != definition.return_type {
            return false;
        }

        for extern_param in &definition.parameters {
            let matching_provider_param = provider_def
                .parameters
                .iter()
                .find(|p| p.name == extern_param.name);

            match matching_provider_param {
                Some(provider_param) if provider_param.param_type == extern_param.param_type => {
                    continue;
                }
                _ => return false,
            }
        }

        true
    }

    fn find_matching_provider<'a>(
        matches: &'a [(ExternalFunctionDefinition, Arc<dyn FunctionProvider>)],
        definition: &ExternalFunctionDefinition,
        name: &str,
    ) -> Result<&'a Arc<dyn FunctionProvider>, RuntimeError> {
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
        self.create_runtime_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::config::ProgramSource;
    use crate::types::{ExternalFunctionDefinition, Parameter, Type};

    #[test]
    fn test_signatures_match_same_order() {
        let provider_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("prefix".to_string(), Type::string()),
            ],
            Type::string(),
        );

        let extern_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("prefix".to_string(), Type::string()),
            ],
            Type::string(),
        );

        assert!(Runtime::signatures_match(&provider_def, &extern_def));
    }

    #[test]
    fn test_signatures_match_different_order() {
        let provider_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("prefix".to_string(), Type::string()),
            ],
            Type::string(),
        );

        let extern_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("prefix".to_string(), Type::string()),
                Parameter::new("message".to_string(), Type::string()),
            ],
            Type::string(),
        );

        assert!(Runtime::signatures_match(&provider_def, &extern_def));
    }

    #[test]
    fn test_signatures_match_different_param_count() {
        let provider_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("prefix".to_string(), Type::string()),
            ],
            Type::string(),
        );

        let extern_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![Parameter::new("message".to_string(), Type::string())],
            Type::string(),
        );

        assert!(!Runtime::signatures_match(&provider_def, &extern_def));
    }

    #[test]
    fn test_signatures_match_different_param_names() {
        let provider_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("prefix".to_string(), Type::string()),
            ],
            Type::string(),
        );

        let extern_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("suffix".to_string(), Type::string()),
            ],
            Type::string(),
        );

        assert!(!Runtime::signatures_match(&provider_def, &extern_def));
    }

    #[test]
    fn test_signatures_match_different_param_types() {
        let provider_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("flag".to_string(), Type::string()),
            ],
            Type::string(),
        );

        let extern_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![
                Parameter::new("message".to_string(), Type::string()),
                Parameter::new("flag".to_string(), Type::boolean()),
            ],
            Type::string(),
        );

        assert!(!Runtime::signatures_match(&provider_def, &extern_def));
    }

    #[test]
    fn test_signatures_match_different_return_types() {
        let provider_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![Parameter::new("message".to_string(), Type::string())],
            Type::string(),
        );

        let extern_def = ExternalFunctionDefinition::new(
            "test_func".to_string(),
            vec![Parameter::new("message".to_string(), Type::string())],
            Type::boolean(),
        );

        assert!(!Runtime::signatures_match(&provider_def, &extern_def));
    }

    #[tokio::test]
    async fn test_struct_registry_populated_after_run() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn main(): Int {
    let p = Point { x: 1, y: 2 }
    return p.x
}
"#;
        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        let result = runtime.run().await.unwrap();
        assert_eq!(result.as_integer().unwrap(), 1);
    }

    #[test]
    fn test_get_struct_returns_none_for_unknown() {
        let runtime = Runtime::builder(ProgramSource::Inline(
            "fn main(): () { return () }".to_string(),
        ))
        .build();
        assert!(runtime.get_struct("Unknown").is_none());
    }

    #[tokio::test]
    async fn test_get_struct_returns_fields_after_run() {
        let code = r#"
struct Task {
    title: String,
    steps: Int,
}
fn main(): () {
    return ()
}
"#;
        let unit = CompilationUnit::from_string(code.to_string());
        let compiler = Compiler::new();
        let compiled = compiler.compile_source(&unit).unwrap();
        let mut runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        for (name, fields) in compiled.struct_definitions() {
            runtime.register_struct(name.clone(), fields.clone());
        }
        let fields = runtime.get_struct("Task").unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "title");
        assert_eq!(fields[1].0, "steps");
    }
}
