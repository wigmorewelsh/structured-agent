use crate::bytecode::{BytecodeFunctionExpr, BytecodeRefs};
use crate::cli::config::{Config, EngineType, McpServerConfig, ProgramSource};
use crate::compiler::{CompilationUnit, CompiledProgram, Compiler};
use crate::gemini::{GeminiConfig, GeminiEngine};
use crate::mcp::McpClient;
use crate::runtime::actor::actor_loop;
use crate::runtime::{Context, ExpressionValue, RuntimeService};
use crate::typecheck::{CheckerAstRef, TypedCheckerAstRef};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, Function, FunctionProvider, LanguageEngine,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use structured_agent_il::CompiledFunction;
use structured_agent_il::Module;
use structured_agent_openai::{HF_BASE_URL, OpenAIEngine};
use structured_agent_runtime::actor::ActorRegistry;
use structured_agent_runtime::symbols::{MetaData, TypeDefinitionKind};
use structured_agent_runtime::{DefinitionPath, SymbolQuery};
use structured_agent_stdlib::{
    actor::ActorModule, equality::EqualityModule, fs::FsModule, io::IoModule,
    iterator::IteratorModule, logic::LogicModule, math::MathModule, messaging::MessagingModule,
    prelude::PreludeModule, unstable::UnstableModule,
};
use tracing::{debug, error};

struct CachedProgram {
    metadata: Arc<MetaData<BytecodeRefs>>,
    main_function: Option<DefinitionPath>,
    extern_registry: HashMap<String, ExternalFunctionDefinition>,
}

pub struct Runtime {
    function_registry: HashMap<String, Arc<dyn ExecutableFunction>>,
    language_engine: Arc<dyn LanguageEngine>,
    compiler: Arc<Compiler>,
    providers: Vec<Arc<dyn FunctionProvider>>,
    program_source: ProgramSource,
    compiled: Arc<OnceLock<Result<CachedProgram, String>>>,
    actor_registry: Arc<ActorRegistry>,
}

pub struct RuntimeBuilder {
    providers: Vec<Arc<dyn FunctionProvider>>,
    language_engine: Option<Arc<dyn LanguageEngine>>,
    compiler: Option<Arc<Compiler>>,
    program_source: ProgramSource,
    mcp_working_dir: Option<String>,
    modules: Vec<Arc<dyn structured_agent_il::Module>>,
}

pub use structured_agent_runtime::RuntimeError;

impl RuntimeBuilder {
    pub fn new(source: ProgramSource) -> Self {
        Self {
            providers: Vec::new(),
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

    pub fn with_module(mut self, module: Arc<dyn Module>) -> Self {
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

    pub async fn with_config(mut self, config: &Config) -> Result<Runtime, String> {
        self = self.with_mcp_server_configs(&config.mcp_servers).await?;

        let engine: Arc<dyn LanguageEngine> = match &config.engine {
            EngineType::Print => Arc::new(crate::types::PrintEngine {}),
            EngineType::OpenAI {
                api_key,
                model,
                base_url,
            } => Arc::new(OpenAIEngine::new(api_key, base_url, model)),
            EngineType::HuggingFace { token, model } => {
                Arc::new(OpenAIEngine::new(token, HF_BASE_URL, model))
            }
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

        self = self
            .with_module(Arc::new(IoModule))
            .with_module(Arc::new(MessagingModule))
            .with_module(Arc::new(FsModule))
            .with_module(Arc::new(IteratorModule))
            .with_module(Arc::new(LogicModule))
            .with_module(Arc::new(MathModule))
            .with_module(Arc::new(EqualityModule))
            .with_module(Arc::new(PreludeModule));

        if config.with_unstable_functions {
            self = self.with_module(Arc::new(UnstableModule));
        }

        Ok(self.build())
    }

    pub fn build(self) -> Runtime {
        let providers = self.providers;

        let function_registry = HashMap::new();

        let default_compiler = self.modules.iter().fold(
            Compiler::new().with_module(Arc::new(ActorModule)),
            |c, m| c.with_module(Arc::clone(m)),
        );

        Runtime {
            function_registry,
            language_engine: self
                .language_engine
                .unwrap_or_else(|| Arc::new(crate::types::PrintEngine {})),
            compiler: self.compiler.unwrap_or_else(|| Arc::new(default_compiler)),
            providers,
            program_source: self.program_source,
            compiled: Arc::new(OnceLock::new()),
            actor_registry: ActorRegistry::new(),
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

    pub fn get_native_function(&self, name: &str) -> Option<Arc<dyn ExecutableFunction>> {
        self.function_registry.get(name).cloned()
    }

    pub fn get_bytecode_function(
        &self,
        name: &DefinitionPath,
    ) -> Option<Arc<dyn ExecutableFunction>> {
        let cached = self.compiled.get()?.as_ref().ok()?;
        let func_def = cached.metadata.functions.get(name)?;
        let body = func_def.body_ref.as_ref()?;
        Some(Arc::new(BytecodeFunctionExpr::new(
            name.clone(),
            body.clone(),
        )))
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

    pub fn dump_il(&self) -> Result<Vec<CompiledFunction>, RuntimeError> {
        let cached = self.ensure_compiled()?;
        let mut functions: Vec<CompiledFunction> = cached
            .metadata
            .functions
            .iter()
            .filter_map(|(name, func_def)| {
                let body = func_def.body_ref.as_ref()?;
                Some(CompiledFunction {
                    name: name.clone(),
                    module_name: None,
                    parameters: body.parameters.clone(),
                    return_type: body.return_type.clone(),
                    instructions: body.instructions.clone(),
                    labels: body.labels.clone(),
                    documentation: body.documentation.clone(),
                    slot_table: body.slot_table.clone(),
                })
            })
            .collect();
        functions.sort_by(|a, b| a.name.to_string().cmp(&b.name.to_string()));
        Ok(functions)
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

        if let Err(e) = runtime
            .map_providers_to_functions(&cached.extern_registry)
            .await
        {
            error!("Failed to map providers to functions: {:?}", e);
            return Err(e);
        }

        if let Some(main_name) = &cached.main_function {
            if let Some(func_def) = cached.metadata.functions.get(main_name) {
                if let Some(main_body) = &func_def.body_ref {
                    debug!("Executing main function");
                    let main_expr = BytecodeFunctionExpr::new(main_name.clone(), main_body.clone());
                    let initial_context = Context::with_runtime_and_handle(
                        Arc::new(runtime) as Arc<dyn RuntimeService>,
                        handle,
                    );
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
        let initial_context =
            Context::with_runtime(Arc::new(self.create_runtime_ref()) as Arc<dyn RuntimeService>);
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

    pub fn get_struct(
        &self,
        type_name: &structured_agent_runtime::symbols::DefinitionPath,
    ) -> Option<Vec<(String, crate::types::Type)>> {
        let cached = self.compiled.get()?.as_ref().ok()?;
        let td = cached.metadata.types.get(type_name)?;
        if let TypeDefinitionKind::Struct { fields, .. } = &td.kind {
            Some(
                fields
                    .iter()
                    .map(|f| (f.name.clone(), field_type_name_to_type(&f.type_name)))
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn get_struct_with_args(
        &self,
        type_name: &structured_agent_runtime::symbols::DefinitionPath,
        args: &[crate::types::Type],
    ) -> Option<Vec<(String, crate::types::Type)>> {
        let cached = self.compiled.get()?.as_ref().ok()?;
        let td = cached.metadata.types.get(type_name)?;
        if let TypeDefinitionKind::Struct {
            fields,
            generic_parameters,
            ..
        } = &td.kind
        {
            let substitution: Vec<(&str, &crate::types::Type)> = generic_parameters
                .iter()
                .zip(args.iter())
                .map(|(gp, ty)| (gp.name.as_str(), ty))
                .collect();
            Some(
                fields
                    .iter()
                    .map(|f| {
                        let ty = substitution
                            .iter()
                            .find(|(k, _)| *k == f.type_name.last_name())
                            .map(|(_, t)| (*t).clone())
                            .unwrap_or_else(|| field_type_name_to_type(&f.type_name));
                        (f.name.clone(), ty)
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn type_to_arrow_datatype(&self, ty: &crate::types::Type) -> arrow::datatypes::DataType {
        self.compiled
            .get()
            .and_then(|c| c.as_ref().ok())
            .map(|c| structured_agent_runtime::type_to_arrow_datatype(ty, c.metadata.as_ref()))
            .unwrap_or(arrow::datatypes::DataType::Null)
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
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
            providers: self.providers.clone(),
            program_source: self.program_source.clone(),
            compiled: Arc::clone(&self.compiled),
            actor_registry: self.actor_registry.clone(),
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
                    .map(|p| format!("{}: {}", p.name, p.param_type))
                    .collect::<Vec<_>>()
                    .join(", ");

                let available_sigs = matches
                    .iter()
                    .map(|(provider_def, _)| {
                        let params = provider_def
                            .parameters
                            .iter()
                            .map(|p| format!("{}: {}", p.name, p.param_type))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("  - fn {}({}) -> {}", name, params, provider_def.return_type)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                RuntimeError::ExecutionError(format!(
                    "No matching provider found for extern function '{}'.\n\nExpected signature:\n  fn {}({}) -> {}\n\nAvailable signatures from providers:\n{}",
                    name, name, expected_params, definition.return_type, available_sigs
                ))
            })
    }

    async fn map_providers_to_functions(
        &mut self,
        extern_registry: &HashMap<String, ExternalFunctionDefinition>,
    ) -> Result<(), RuntimeError> {
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

        for (name, definition) in extern_registry {
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
}

impl RuntimeService for Runtime {
    fn get_native_function(&self, name: &str) -> Option<Arc<dyn ExecutableFunction>> {
        self.function_registry.get(name).cloned()
    }

    fn actor_registry(&self) -> Arc<ActorRegistry> {
        self.actor_registry.clone()
    }

    fn get_bytecode_ref(&self, name: &DefinitionPath) -> Option<structured_agent_il::BytecodeRef> {
        let cached = self.compiled.get()?.as_ref().ok()?;
        let func_def = cached.metadata.functions.get(name)?;
        func_def.body_ref.as_ref().cloned()
    }

    fn engine(&self) -> &dyn LanguageEngine {
        self.language_engine.as_ref()
    }

    fn type_to_arrow_datatype(
        &self,
        ty: &structured_agent_runtime::Type,
    ) -> arrow::datatypes::DataType {
        self.compiled
            .get()
            .and_then(|c| c.as_ref().ok())
            .map(|c| structured_agent_runtime::type_to_arrow_datatype(ty, c.metadata.as_ref()))
            .unwrap_or(arrow::datatypes::DataType::Null)
    }

    fn get_struct(&self, type_name: &DefinitionPath) -> Option<Vec<(String, crate::types::Type)>> {
        Runtime::get_struct(self, type_name)
    }

    fn get_struct_with_args(
        &self,
        type_name: &DefinitionPath,
        args: &[crate::types::Type],
    ) -> Option<Vec<(String, crate::types::Type)>> {
        Runtime::get_struct_with_args(self, type_name, args)
    }

    fn spawn_actor(
        &self,
        mailbox: structured_agent_runtime::ActorMailboxReceiver,
        context: crate::runtime::Context,
    ) {
        let runtime: Arc<dyn RuntimeService> = Arc::new(self.clone());
        tokio::spawn(actor_loop(mailbox, context, runtime));
    }
}

impl Clone for Runtime {
    fn clone(&self) -> Self {
        self.create_runtime_ref()
    }
}

fn field_type_name_to_type(
    type_name: &structured_agent_runtime::symbols::DefinitionPath,
) -> crate::types::Type {
    crate::types::Type::Named(type_name.clone())
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
                name: func_def.name.last_name().to_string(),
                parameters: params.clone(),
                return_type: return_type.clone(),
                type_params: type_params.clone(),
                is_pub: true,
                span: crate::types::Span::dummy(),
            };
            if let Ok(ext_def) =
                crate::compiler::compile_external_function(&ast_ext, &func_def.name.module_prefix())
            {
                extern_registry.insert(ext_def.name.clone(), ext_def);
            }
        }
    }

    let metadata = Arc::new(compiled.metadata);

    Ok(CachedProgram {
        metadata,
        main_function,
        extern_registry,
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
        use nonempty::NonEmpty;
        use structured_agent_runtime::symbols::DefinitionPath;
        let runtime = Runtime::builder(ProgramSource::Inline(
            "fn main(): () { return () }".to_string(),
        ))
        .build();
        let type_name = DefinitionPath::for_type(
            DefinitionPath::for_module(NonEmpty::new("test".to_string())),
            "Unknown",
        );
        assert!(runtime.get_struct(&type_name).is_none());
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
        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        runtime.run().await.unwrap();
        let task_type_name = runtime
            .compiled
            .get()
            .unwrap()
            .as_ref()
            .unwrap()
            .metadata
            .types
            .keys()
            .find(|tn| tn.last_name() == "Task")
            .cloned()
            .unwrap();
        let fields = runtime.get_struct(&task_type_name).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].0, "title");
        assert_eq!(fields[1].0, "steps");
    }
}
