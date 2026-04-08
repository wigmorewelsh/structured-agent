use crate::bytecode::{BytecodeFunctionExpr, BytecodeRefs};
use crate::cli::config::{Config, EngineType, McpServerConfig, ProgramSource};
use crate::compiler::{CompilationUnit, CompiledProgram, Compiler};
use crate::gemini::{GeminiConfig, GeminiEngine};
use crate::mcp::McpClient;
use crate::runtime::{Context, ExpressionValue, NativeFunctionProvider};
use crate::typecheck::{CheckerAstRef, TypedCheckerAstRef};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, Function, FunctionProvider, LanguageEngine,
    NativeFunction,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use structured_agent_runtime::symbols::{MetaData, TypeDefinitionKind};
use structured_agent_runtime::{FunctionName, FunctionNameKind, Module, ModuleName, SymbolQuery};
use structured_agent_stdlib::{
    fs::FsModule, io::IoModule, messaging::MessagingModule, unstable::UnstableModule,
};
use tracing::{debug, error};

struct CachedProgram {
    metadata: Arc<MetaData<BytecodeRefs>>,
    main_function: Option<FunctionName>,
    extern_registry: HashMap<String, ExternalFunctionDefinition>,
    aliases: HashMap<String, FunctionName>,
}

pub struct Runtime {
    function_registry: HashMap<String, Arc<dyn ExecutableFunction>>,
    external_function_registry: HashMap<String, ExternalFunctionDefinition>,
    struct_registry: HashMap<String, Vec<(String, crate::types::Type)>>,
    language_engine: Arc<dyn LanguageEngine>,
    compiler: Arc<Compiler>,
    providers: Vec<Arc<dyn FunctionProvider>>,
    program_source: ProgramSource,
    compiled: Arc<OnceLock<Result<CachedProgram, String>>>,
}

pub struct RuntimeBuilder {
    providers: Vec<Arc<dyn FunctionProvider>>,
    native_provider: NativeFunctionProvider,
    language_engine: Option<Arc<dyn LanguageEngine>>,
    compiler: Option<Arc<Compiler>>,
    program_source: ProgramSource,
    mcp_working_dir: Option<String>,
    modules: Vec<Arc<dyn Module>>,
}

pub use structured_agent_runtime::RuntimeError;

impl RuntimeBuilder {
    pub fn new(source: ProgramSource) -> Self {
        Self {
            providers: Vec::new(),
            native_provider: NativeFunctionProvider::new(),
            language_engine: None,
            compiler: None,
            program_source: source,
            mcp_working_dir: None,
            modules: Vec::new(),
        }
    }

    pub fn with_mcp_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.mcp_working_dir = Some(dir.into());
        self
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

    pub fn with_module(mut self, module: Arc<dyn Module>) -> Self {
        for func in module.functions() {
            self.native_provider.add_dyn_function(func);
        }
        self.modules.push(module);
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
            let working_dir = config
                .working_dir
                .clone()
                .or_else(|| self.mcp_working_dir.clone());
            match McpClient::new_stdio(&config.command, config.args.clone(), working_dir).await {
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
            self = self.with_module(Arc::new(IoModule));
        }

        if config.with_unstable_functions {
            self = self.with_module(Arc::new(UnstableModule));
        }

        if config.with_acp_functions {
            self = self
                .with_module(Arc::new(MessagingModule))
                .with_module(Arc::new(FsModule));
        }

        Ok(self.build())
    }

    pub fn build(self) -> Runtime {
        let native_provider_rc = Arc::new(self.native_provider);
        let mut providers = self.providers;
        providers.push(native_provider_rc.clone());

        let function_registry = native_provider_rc.native_functions.clone();

        let default_compiler = self
            .modules
            .iter()
            .fold(Compiler::new(), |c, m| c.with_module(Arc::clone(m)));

        Runtime {
            function_registry,
            external_function_registry: HashMap::new(),
            struct_registry: HashMap::new(),
            language_engine: self
                .language_engine
                .unwrap_or_else(|| Arc::new(crate::types::PrintEngine {})),
            compiler: self.compiler.unwrap_or_else(|| Arc::new(default_compiler)),
            providers,
            program_source: self.program_source,
            compiled: Arc::new(OnceLock::new()),
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

    pub fn get_function(&self, name: &str) -> Option<Arc<dyn ExecutableFunction>> {
        if let Some(func) = self.function_registry.get(name) {
            return Some(func.clone());
        }
        let cached = self.compiled.get()?.as_ref().ok()?;
        if let Some(canonical) = cached.aliases.get(name)
            && let Some(func_def) = cached.metadata.functions.get(canonical)
            && let Some(body) = &func_def.body_ref
        {
            return Some(Arc::new(BytecodeFunctionExpr::new(
                canonical.clone(),
                body.clone(),
            )));
        }
        if let Some(func_name) = FunctionName::parse(name) {
            if let Some(func_def) = cached.metadata.functions.get(&func_name)
                && let Some(body) = &func_def.body_ref
            {
                return Some(Arc::new(BytecodeFunctionExpr::new(func_name, body.clone())));
            }
        }
        if !name.contains("::") {
            let main_func_name = FunctionName {
                name: name.to_string(),
                module: ModuleName::from_str("main"),
                kind: FunctionNameKind::Function,
            };
            if let Some(func_def) = cached.metadata.functions.get(&main_func_name)
                && let Some(body) = &func_def.body_ref
            {
                return Some(Arc::new(BytecodeFunctionExpr::new(
                    main_func_name,
                    body.clone(),
                )));
            }
        }
        None
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
        self.ensure_compiled().map(|_| ())
    }

    pub async fn run(&self) -> Result<ExpressionValue, RuntimeError> {
        self.run_with_handle(crate::runtime::AgentHandle::detached())
            .await
    }

    pub async fn run_with_handle(
        &self,
        handle: crate::runtime::AgentHandle,
    ) -> Result<ExpressionValue, RuntimeError> {
        debug!("Starting program execution");

        let cached = self.ensure_compiled()?;
        let mut runtime = self.create_runtime_ref();

        for (name, def) in &cached.extern_registry {
            runtime
                .external_function_registry
                .insert(name.clone(), def.clone());
        }

        if let Err(e) = runtime.map_providers_to_functions().await {
            error!("Failed to map providers to functions: {:?}", e);
            return Err(e);
        }

        if let Some(main_name) = &cached.main_function {
            if let Some(func_def) = cached.metadata.functions.get(main_name) {
                if let Some(main_body) = &func_def.body_ref {
                    debug!("Executing main function");
                    let main_expr = BytecodeFunctionExpr::new(main_name.clone(), main_body.clone());
                    let initial_context =
                        Context::with_runtime_and_handle(Arc::new(runtime), handle);
                    match main_expr.execute(initial_context, vec![]).await {
                        Ok((_, result)) => {
                            debug!("Program execution completed successfully");
                            debug!("Result type: {}", result.value.type_name());
                            Ok(result.value)
                        }
                        Err(e) => {
                            error!("Runtime execution failed: {:?}", e);
                            Err(RuntimeError::ExecutionError(e))
                        }
                    }
                } else {
                    error!("No main function body found");
                    Err(RuntimeError::FunctionNotFound("main".to_string()))
                }
            } else {
                error!("No main function found in program");
                Err(RuntimeError::FunctionNotFound("main".to_string()))
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

    pub fn get_struct(&self, name: &str) -> Option<Vec<(String, crate::types::Type)>> {
        if let Some(fields) = self.struct_registry.get(name) {
            return Some(fields.clone());
        }
        let cached = self.compiled.get()?.as_ref().ok()?;
        cached.metadata.type_by_name(name).and_then(|td| {
            if let TypeDefinitionKind::Struct { fields } = &td.kind {
                Some(
                    fields
                        .iter()
                        .map(|f| (f.name.clone(), field_type_name_to_type(&f.type_name)))
                        .collect(),
                )
            } else {
                None
            }
        })
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

    fn ensure_compiled(&self) -> Result<&CachedProgram, RuntimeError> {
        self.compiled
            .get_or_init(|| self.compile().and_then(build_cached_program))
            .as_ref()
            .map_err(|e| RuntimeError::ExecutionError(e.clone()))
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
            compiled: Arc::clone(&self.compiled),
        }
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

fn field_type_name_to_type(
    type_name: &structured_agent_runtime::symbols::TypeName,
) -> crate::types::Type {
    match type_name.name.as_str() {
        "Int" => crate::types::Type::int(),
        "String" => crate::types::Type::string(),
        "Boolean" => crate::types::Type::boolean(),
        "Unit" => crate::types::Type::unit(),
        _ => crate::types::Type::Struct(type_name.name.clone()),
    }
}

fn build_cached_program(compiled: CompiledProgram) -> Result<CachedProgram, String> {
    let main_function = compiled.main_function_name().cloned();

    let mut extern_registry = HashMap::new();
    for func_def in compiled.metadata.all_functions() {
        if let TypedCheckerAstRef::Other(CheckerAstRef::ExternalFn {
            params,
            return_type,
            type_params,
            ..
        }) = &func_def.ast_ref
            && func_def.source_ref.1 != crate::types::Span::dummy()
        {
            let ast_ext = crate::ast::ExternalFunction {
                name: func_def.name.to_string(),
                parameters: params.clone(),
                return_type: return_type.clone(),
                type_params: type_params.clone(),
                is_pub: true,
                span: crate::types::Span::dummy(),
            };
            if let Ok(ext_def) = crate::compiler::compile_external_function(&ast_ext) {
                extern_registry.insert(ext_def.name.clone(), ext_def);
            }
        }
    }

    let mut aliases = HashMap::new();
    for module in compiled.metadata.modules.values() {
        for (alias, qualified) in &module.use_aliases {
            let canonical = FunctionName::parse(qualified).unwrap_or_else(|| FunctionName {
                name: qualified.to_string(),
                module: ModuleName::from_str(""),
                kind: FunctionNameKind::Function,
            });
            aliases.insert(alias.clone(), canonical);
        }
    }

    let metadata = Arc::new(compiled.metadata);

    Ok(CachedProgram {
        metadata,
        main_function,
        extern_registry,
        aliases,
    })
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
        use structured_agent_runtime::symbols::TypeDefinitionKind;
        for type_def in compiled.metadata.all_types() {
            if let TypeDefinitionKind::Struct { fields } = &type_def.kind {
                let converted: Vec<(String, crate::types::Type)> = fields
                    .iter()
                    .map(|f| (f.name.clone(), field_type_name_to_type(&f.type_name)))
                    .collect();
                runtime.register_struct(type_def.name.name.clone(), converted);
            }
        }
        let fields = runtime.get_struct("Task").unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "title");
        assert_eq!(fields[1].0, "steps");
    }
}
