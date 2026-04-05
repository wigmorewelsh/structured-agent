pub(crate) mod discovery;
pub mod parser;
pub(crate) mod sigs;
pub(crate) mod wiring;
use wiring::{lower_typed_module, resolve_vtables};

use crate::analysis::{
    AnalysisRunner, ConstantConditionAnalyzer, DuplicateInjectionAnalyzer, EmptyBlockAnalyzer,
    EmptyFunctionAnalyzer, InfiniteLoopAnalyzer, OverwrittenValueAnalyzer,
    PlaceholderOveruseAnalyzer, ReachabilityAnalyzer, RedundantSelectAnalyzer,
    UnusedExpressionAnalyzer, UnusedReturnValueAnalyzer, UnusedVariableAnalyzer,
    VariableShadowingAnalyzer,
};
use crate::ast::{Module, SigFunction};
use crate::bytecode::{BytecodeCompiler, CompiledFunction};
use crate::diagnostics::{DiagnosticManager, DiagnosticReporter};
use crate::il_analysis::{
    IlAnalysisRunner, IlWarning, VariableAllocationAnalyzer, VariableDropAnalyzer,
};
use crate::typecheck::TypeChecker;
use crate::typecheck::checker::{FunctionKind, ModuleVisibility};
use crate::typed_ast;
use crate::types::{ExternalFunctionDefinition, FileId, Parameter, Type};

use combine::Parser as CombineParser;
use combine::stream::{easy, position};
use discovery::{Discoverer, FileDiscoverer, InMemoryDiscoverer, discover};
use sigs::{SigTable, collect_sigs, sigs_visible_to_module};
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::types::Module as RuntimeModule;

use tracing::{debug, error, warn};

#[derive(Debug, Clone)]
pub struct CompilationUnit {
    source: String,
    name: String,
    path: Option<String>,
}

impl CompilationUnit {
    pub fn from_string(source: String) -> Self {
        Self {
            name: "main".to_string(),
            source,
            path: None,
        }
    }

    pub fn from_file(path: String, source: String) -> Self {
        Self {
            name: path.clone(),
            source,
            path: Some(path),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

struct ModuleArtifact {
    functions: Vec<CompiledFunction>,
    external_functions: Vec<ExternalFunctionDefinition>,
    struct_definitions: Vec<(String, Vec<(String, Type)>)>,
    sig_definitions: Vec<(String, Vec<SigFunction>)>,
    use_aliases: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct CompiledProgram {
    functions: HashMap<String, CompiledFunction>,
    external_functions: HashMap<String, ExternalFunctionDefinition>,
    struct_definitions: HashMap<String, Vec<(String, Type)>>,
    sig_definitions: HashMap<String, Vec<SigFunction>>,
    module_visibility: ModuleVisibility,
    pending_aliases: Vec<(String, String)>,
    use_aliases: Vec<(String, String)>,
    main_function: Option<String>,
    source_path: Option<String>,
}

impl Default for CompiledProgram {
    fn default() -> Self {
        Self::new()
    }
}

impl CompiledProgram {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            external_functions: HashMap::new(),
            struct_definitions: HashMap::new(),
            sig_definitions: HashMap::new(),
            module_visibility: HashMap::new(),
            pending_aliases: Vec::new(),
            use_aliases: Vec::new(),
            main_function: None,
            source_path: None,
        }
    }

    pub fn with_source_path(mut self, path: Option<String>) -> Self {
        self.source_path = path;
        self
    }

    fn with_module_visibility(mut self, visibility: ModuleVisibility) -> Self {
        self.module_visibility = visibility;
        self
    }

    pub fn source_path(&self) -> Option<&str> {
        self.source_path.as_deref()
    }

    pub fn main_function(&self) -> Option<&CompiledFunction> {
        self.main_function
            .as_ref()
            .and_then(|name| self.functions.get(name))
    }

    pub fn functions(&self) -> &HashMap<String, CompiledFunction> {
        &self.functions
    }

    pub fn external_functions(&self) -> &HashMap<String, ExternalFunctionDefinition> {
        &self.external_functions
    }

    pub fn struct_definitions(&self) -> &HashMap<String, Vec<(String, Type)>> {
        &self.struct_definitions
    }

    pub fn sig_definitions(&self) -> &HashMap<String, Vec<SigFunction>> {
        &self.sig_definitions
    }

    pub fn module_visibility(&self) -> &ModuleVisibility {
        &self.module_visibility
    }

    fn merge(&mut self, artifact: ModuleArtifact) {
        for f in artifact.functions {
            let name = f.name.clone();
            if name == "main" {
                self.main_function = Some(name.clone());
            }
            self.functions.insert(name, f);
        }
        for f in artifact.external_functions {
            self.external_functions.insert(f.name.clone(), f);
        }
        for (name, fields) in artifact.struct_definitions {
            self.struct_definitions.insert(name, fields);
        }
        for (name, functions) in artifact.sig_definitions {
            self.sig_definitions.insert(name, functions);
        }
        for (alias, qualified) in artifact.use_aliases {
            self.pending_aliases.push((alias, qualified));
        }
    }

    fn apply_pending_aliases(&mut self) {
        let aliases = std::mem::take(&mut self.pending_aliases);
        self.use_aliases = aliases.clone();
        for (alias, qualified) in aliases {
            if let Some(f) = self.functions.get(&qualified).cloned() {
                self.functions.insert(alias, f);
            }
        }
    }

    pub fn use_aliases(&self) -> &[(String, String)] {
        &self.use_aliases
    }
}

pub struct Compiler {
    parser: CodespanParser,
    modules: HashMap<String, Arc<dyn RuntimeModule>>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            parser: CodespanParser::new(),
            modules: HashMap::new(),
        }
    }

    pub fn with_module(mut self, module: Arc<dyn RuntimeModule>) -> Self {
        self.modules.insert(module.name().to_string(), module);
        self
    }

    pub fn compile_source(&self, unit: &CompilationUnit) -> Result<CompiledProgram, String> {
        let mut sources = HashMap::new();
        sources.insert(unit.name().to_string(), unit.source().to_string());
        let discoverer = InMemoryDiscoverer::new(sources);
        self.compile(
            unit.name(),
            unit.source(),
            unit.path().map(String::from),
            &discoverer,
        )
    }

    pub fn compile_file(&self, entry_path: &str) -> Result<CompiledProgram, String> {
        let entry_source = std::fs::read_to_string(entry_path)
            .map_err(|e| format!("Failed to read {}: {}", entry_path, e))?;
        self.compile(
            entry_path,
            &entry_source,
            Some(entry_path.to_string()),
            &FileDiscoverer,
        )
    }

    fn compile(
        &self,
        entry_path: &str,
        entry_source: &str,
        source_path: Option<String>,
        discoverer: &impl Discoverer,
    ) -> Result<CompiledProgram, String> {
        debug!("Compiling: {}", entry_path);

        let parser = &self.parser;
        let mut diagnostics = DiagnosticManager::new();

        let native_names: std::collections::HashSet<String> =
            self.modules.keys().cloned().collect();
        let modules = discover(
            entry_path,
            entry_source,
            discoverer,
            &native_names,
            |path, source| {
                let unit = CompilationUnit::from_file(path.to_string(), source.to_string());
                let file_id = diagnostics.add_file(path.to_string(), source.to_string());
                let reporter = diagnostics.reporter().clone();
                parser
                    .parse(&unit, file_id, &reporter)
                    .map(|m| (file_id, m))
            },
        )?;

        let sig_table: SigTable = collect_sigs(&modules, &self.modules);

        let mut typed_modules: HashMap<String, typed_ast::Module> = HashMap::new();

        for parsed in &modules {
            let reporter = diagnostics.reporter().clone();
            match type_check_module(parsed, &sig_table) {
                Ok((typed_module, _)) => {
                    typed_modules.insert(parsed.name.clone(), typed_module);
                }
                Err(e) => {
                    error!("Type checking failed: {}", e);
                    if let Err(io_err) = reporter.emit_type_error(&e) {
                        eprintln!("Failed to emit type error: {}", io_err);
                    }
                    return Err(format!("In {}: Type error: {}", parsed.name, e));
                }
            }

            for warning in analyse_module(parsed) {
                if let Err(io_err) = reporter.emit_diagnostic(&warning.to_diagnostic()) {
                    eprintln!("Failed to emit warning: {}", io_err);
                }
            }
        }

        let vtables = resolve_vtables(&modules, &sig_table);
        for parsed in &modules {
            if let Some(vtable) = vtables.get(&parsed.name)
                && let Some(typed_module) = typed_modules.get_mut(&parsed.name)
            {
                lower_typed_module(typed_module, vtable);
            }
        }

        let mut compiled = CompiledProgram::new()
            .with_source_path(source_path)
            .with_module_visibility(sig_table.visibility.clone());

        for parsed in &modules {
            let prefix = (!parsed.is_entry).then_some(parsed.name.as_str());
            let typed_module = typed_modules
                .get(&parsed.name)
                .expect("typed module missing");
            let artifact = emit_module(typed_module, prefix)?;
            compiled.merge(artifact);
        }

        compiled.apply_pending_aliases();

        let il_reporter = diagnostics.reporter().clone();
        for warning in analyse_il(compiled.functions()) {
            if let Err(io_err) = il_reporter.emit_diagnostic(&warning.to_diagnostic()) {
                eprintln!("Failed to emit IL warning: {}", io_err);
            }
        }

        Ok(compiled)
    }
}

fn type_check_module(
    parsed: &discovery::ParsedModule,
    sig_table: &SigTable,
) -> Result<(crate::typed_ast::Module, HashMap<String, FunctionKind>), crate::typecheck::TypeError>
{
    let external_sigs = if parsed.is_entry {
        sigs_visible_to_module(&parsed.module, sig_table)
    } else {
        sig_table.external_sigs.clone()
    };
    let mut checker = TypeChecker::new();
    checker.check_module_with_external_sigs(
        &parsed.module,
        parsed.file_id,
        &external_sigs,
        &sig_table.visibility,
        &sig_table.sig_definitions,
    )
}

fn analyse_module(parsed: &discovery::ParsedModule) -> Vec<crate::analysis::Warning> {
    let warnings = build_analysis_runner().run(&parsed.module, parsed.file_id);
    if !warnings.is_empty() {
        warn!("Analysis found {} warnings", warnings.len());
    }
    warnings
}

fn emit_module(module: &typed_ast::Module, prefix: Option<&str>) -> Result<ModuleArtifact, String> {
    let mut artifact = ModuleArtifact {
        functions: Vec::new(),
        external_functions: Vec::new(),
        struct_definitions: Vec::new(),
        sig_definitions: Vec::new(),
        use_aliases: Vec::new(),
    };

    let compiler = BytecodeCompiler::new();

    for definition in &module.definitions {
        match definition {
            typed_ast::Definition::Function(f) => {
                let mut compiled = compiler.compile_to_bytecode(f)?;
                if let Some(p) = prefix {
                    compiled.name = format!("{}::{}", p, compiled.name);
                }
                debug!("Emitting function: {}", compiled.name);
                compiled.module_name = prefix.map(str::to_string);
                artifact.functions.push(compiled);
            }
            typed_ast::Definition::ExternalFunction(f) => {
                let mut f = f.clone();
                if let Some(p) = prefix {
                    f.name = format!("{}::{}", p, f.name);
                }
                debug!("Emitting external function: {}", f.name);
                artifact
                    .external_functions
                    .push(compile_external_function(&f)?);
            }
            typed_ast::Definition::Struct(s) => {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), ast_type_to_type(&f.field_type)))
                    .collect();
                artifact.struct_definitions.push((s.name.clone(), fields));
            }
            typed_ast::Definition::Signature {
                name, functions, ..
            } => {
                artifact
                    .sig_definitions
                    .push((name.clone(), functions.clone()));
            }
            typed_ast::Definition::Use { path, alias, .. } if prefix.is_none() => {
                if path.len() >= 2 {
                    let qualified = format!("{}::{}", path[0], path.last().unwrap());
                    let local = alias
                        .clone()
                        .unwrap_or_else(|| path.last().unwrap().clone());
                    artifact.use_aliases.push((local, qualified));
                }
            }
            typed_ast::Definition::Use { .. }
            | typed_ast::Definition::ModuleHeader { .. }
            | typed_ast::Definition::ModuleBinding { .. }
            | typed_ast::Definition::WiringSite { .. }
            | typed_ast::Definition::Trait { .. }
            | typed_ast::Definition::TraitImpl { .. } => {}
        }
    }

    Ok(artifact)
}

pub(crate) fn runtime_type_to_ast(ty: &structured_agent_runtime::types::Type) -> crate::ast::Type {
    use structured_agent_runtime::types::Type as RT;
    match ty {
        RT::String => crate::ast::Type::String,
        RT::Boolean => crate::ast::Type::Boolean,
        RT::Int => crate::ast::Type::Int,
        RT::Unit => crate::ast::Type::Unit,
        RT::List(inner) => crate::ast::Type::List(Box::new(runtime_type_to_ast(inner))),
        RT::Option(inner) => crate::ast::Type::Option(Box::new(runtime_type_to_ast(inner))),
        RT::Struct(name) => crate::ast::Type::Struct(name.clone()),
        RT::Generic(name) => crate::ast::Type::Generic(name.clone()),
    }
}

pub fn compile_external_function(
    ast_ext_func: &crate::ast::ExternalFunction,
) -> Result<ExternalFunctionDefinition, String> {
    let parameters = ast_ext_func
        .parameters
        .iter()
        .map(|p| Parameter::new(p.name.clone(), ast_type_to_type(&p.param_type)))
        .collect();
    Ok(ExternalFunctionDefinition::new(
        ast_ext_func.name.clone(),
        parameters,
        ast_type_to_type(&ast_ext_func.return_type),
    ))
}

fn ast_type_to_type(ast_type: &crate::ast::Type) -> Type {
    match ast_type {
        crate::ast::Type::Unit => Type::unit(),
        crate::ast::Type::Boolean => Type::boolean(),
        crate::ast::Type::String => Type::string(),
        crate::ast::Type::List(inner) => Type::list(ast_type_to_type(inner)),
        crate::ast::Type::Option(inner) => Type::option(ast_type_to_type(inner)),
        crate::ast::Type::Int => Type::int(),
        crate::ast::Type::Struct(name) => Type::Struct(name.clone()),
        crate::ast::Type::Generic(name) => Type::Struct(name.clone()),
    }
}

fn analyse_il(functions: &HashMap<String, CompiledFunction>) -> Vec<IlWarning> {
    let mut runner = IlAnalysisRunner::new()
        .with_analyzer(Box::new(VariableAllocationAnalyzer::new()))
        .with_analyzer(Box::new(VariableDropAnalyzer::new()));
    functions.values().flat_map(|f| runner.run(f)).collect()
}

fn build_analysis_runner() -> AnalysisRunner {
    AnalysisRunner::new()
        .with_analyzer(Box::new(UnusedVariableAnalyzer::new()))
        .with_analyzer(Box::new(ReachabilityAnalyzer::new()))
        .with_analyzer(Box::new(InfiniteLoopAnalyzer::new()))
        .with_analyzer(Box::new(EmptyBlockAnalyzer::new()))
        .with_analyzer(Box::new(EmptyFunctionAnalyzer::new()))
        .with_analyzer(Box::new(DuplicateInjectionAnalyzer::new()))
        .with_analyzer(Box::new(PlaceholderOveruseAnalyzer::new()))
        .with_analyzer(Box::new(RedundantSelectAnalyzer::new()))
        .with_analyzer(Box::new(ConstantConditionAnalyzer::new()))
        .with_analyzer(Box::new(VariableShadowingAnalyzer::new()))
        .with_analyzer(Box::new(OverwrittenValueAnalyzer::new()))
        .with_analyzer(Box::new(UnusedReturnValueAnalyzer::new()))
        .with_analyzer(Box::new(UnusedExpressionAnalyzer::new()))
}

pub(crate) struct CodespanParser {}

impl CodespanParser {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) fn parse(
        &self,
        unit: &CompilationUnit,
        file_id: FileId,
        reporter: &DiagnosticReporter,
    ) -> Result<Module, String> {
        let stream = easy::Stream(position::Stream::with_positioner(
            unit.source(),
            position::IndexPositioner::new(),
        ));

        parser::parse_program(file_id)
            .parse(stream)
            .map(|(module, _)| {
                debug!("Parsed {} definitions", module.definitions.len());
                module
            })
            .map_err(|e| {
                let error_str = format!("{}", e);
                error!("Parse error at {}: {}", e.position, error_str);
                let clean = error_str.lines().skip(1).collect::<Vec<_>>().join("\n");
                if let Err(io_err) =
                    reporter.emit_parse_error(file_id, &clean, Some((e.position, e.position + 1)))
                {
                    eprintln!("Failed to emit parse error: {}", io_err);
                }
                "Parse error".to_string()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{CompilationUnit, Compiler};
    use crate::cli::config::ProgramSource;
    use crate::runtime::{ExpressionValue, Runtime};

    async fn run_source(source: &str, expected: &str) {
        let result = Runtime::builder(ProgramSource::Inline(source.to_string()))
            .build()
            .run()
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), expected);
    }

    #[tokio::test]
    async fn test_new_architecture_end_to_end() {
        run_source(
            r#"
fn greet(name: String): () {
    "Hello, "!
    name!
    "!"!
}

fn main(): String {
    "Starting test program"!
    let greeting_name = "World"
    let result = greet(greeting_name)
    "Test completed"!
}
"#,
            "Test completed",
        )
        .await;
    }

    #[tokio::test]
    async fn test_select_statement_end_to_end() {
        run_source(
            r#"
fn add(a: String, b: String): String {
    "Adding numbers"
}

fn subtract(a: String, b: String): String {
    "Subtracting numbers"
}

fn calculator(x: String, y: String): String {
    let result = select {
        add(x, y) as sum => sum,
        subtract(x, y) as diff => diff
    }
    result
}

fn main(): String {
    let result = calculator("5", "3")
    result!
}
"#,
            "<calculator>\n    <param name=\"x\">5</param>\n    <param name=\"y\">3</param>\n    <result>\n    ## calculator\n    </result>\n</calculator>",
        )
        .await;
    }

    #[test]
    fn test_compile_project_two_files() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let lib_path = dir.join("greetlib.sa");
        let main_path = dir.join("mainproj.sa");

        std::fs::File::create(&lib_path)
            .unwrap()
            .write_all(
                b"pub fn greet(name: String): String {\n    return \"hello\"\n}\n\nfn internal(x: String): String {\n    return \"secret\"\n}\n",
            )
            .unwrap();
        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(
                b"use greetlib::greet\n\nfn main(): String {\n    return greet(\"world\")\n}\n",
            )
            .unwrap();

        let compiled = Compiler::new()
            .compile_file(main_path.to_str().unwrap())
            .expect("compile_file failed");

        assert!(compiled.functions().contains_key("main"));
        assert!(compiled.functions().contains_key("greetlib::greet"));
        assert!(compiled.functions().contains_key("greetlib::internal"));
        assert_eq!(
            compiled.module_visibility().get("greetlib::greet"),
            Some(&true)
        );
        assert_eq!(
            compiled.module_visibility().get("greetlib::internal"),
            Some(&false)
        );
    }

    #[test]
    fn test_compile_project_sig_stored() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let path = dir.join("sigtest_main.sa");

        std::fs::File::create(&path)
            .unwrap()
            .write_all(
                b"sig Greeter {\n    fn greet(name: String): String\n}\n\nfn main(): () {}\n",
            )
            .unwrap();

        let compiled = Compiler::new()
            .compile_file(path.to_str().unwrap())
            .expect("compile_file failed");

        assert!(compiled.sig_definitions().contains_key("Greeter"));
        assert_eq!(compiled.sig_definitions()["Greeter"].len(), 1);
        assert_eq!(compiled.sig_definitions()["Greeter"][0].name, "greet");
    }

    #[test]
    fn test_control_flow_analysis_warnings() {
        let source = r#"
fn test_unused(): () {
    let unused_var = "never used"
    "done"!
}

fn test_unreachable(): String {
    return "early"
    "unreachable"!
}

fn test_infinite(): () {
    while true {
        "looping forever"!
    }
    "never reached"!
}

fn main(): () {
    "main"!
}
"#;
        let compiled = Compiler::new()
            .compile_source(&CompilationUnit::from_string(source.to_string()))
            .unwrap();
        assert_eq!(compiled.functions().len(), 4);
    }

    #[tokio::test]
    async fn test_simple_function() {
        run_source(
            r#"
fn add(a: String, b: String): String {
    return "result"
}

fn main(): String {
    return add("1", "2")
}
"#,
            "result",
        )
        .await;
    }

    #[tokio::test]
    async fn test_multi_function() {
        run_source(
            r#"
fn greet(name: String): () {
    "Hello, "!
    name!
}

fn main(): String {
    greet("World")
    "Done"!
}
"#,
            "Done",
        )
        .await;
    }

    #[tokio::test]
    async fn test_unit_literal_end_to_end() {
        let result = Runtime::builder(ProgramSource::Inline(
            "fn main(): () {\n    return ()\n}\n".to_string(),
        ))
        .build()
        .run()
        .await
        .unwrap();
        assert_eq!(result, ExpressionValue::unit());
    }

    #[tokio::test]
    async fn test_if_else_expression_end_to_end() {
        run_source(
            r#"
fn choose_message(ready: Boolean): String {
    return if ready { "System ready" } else { "System not ready" }
}

fn main(): String {
    let message = choose_message(true)
    message!
}
"#,
            "<choose_message>\n    <param name=\"ready\">true</param>\n    <result>\n    System ready\n    </result>\n</choose_message>",
        )
        .await;
    }

    #[test]
    fn test_cross_module_pub_fn_type_checks_ok() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let lib_path = dir.join("vislib.sa");
        let main_path = dir.join("vismain.sa");

        std::fs::File::create(&lib_path)
            .unwrap()
            .write_all(b"pub fn greet(name: String): String {\n    return \"hello\"\n}\n")
            .unwrap();
        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(
                b"use vislib::greet\n\nfn main(): String {\n    return greet(\"world\")\n}\n",
            )
            .unwrap();

        assert!(
            Compiler::new()
                .compile_file(main_path.to_str().unwrap())
                .is_ok()
        );
    }

    #[test]
    fn test_cross_module_private_fn_type_check_fails() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let lib_path = dir.join("privlib.sa");
        let main_path = dir.join("privmain.sa");

        std::fs::File::create(&lib_path)
            .unwrap()
            .write_all(b"fn secret(name: String): String {\n    return \"secret\"\n}\n\npub fn public_fn(): String {\n    return \"ok\"\n}\n")
            .unwrap();
        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(b"use privlib::secret\n\nfn main(): String {\n    return secret(\"x\")\n}\n")
            .unwrap();

        let err = Compiler::new()
            .compile_file(main_path.to_str().unwrap())
            .unwrap_err();
        assert!(
            err.contains("private") || err.contains("PrivateFunction"),
            "error should mention private: {}",
            err
        );
    }

    #[test]
    fn test_vtable_populated_from_named_sig() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let storage_path = dir.join("vtstore.sa");
        let tasks_path = dir.join("vttasks.sa");
        let main_path = dir.join("vtmain.sa");

        std::fs::File::create(&storage_path)
            .unwrap()
            .write_all(b"pub fn read(): String {\n    return \"data\"\n}\n")
            .unwrap();

        std::fs::File::create(&tasks_path)
            .unwrap()
            .write_all(b"mod vttasks(io: vtstore::Store)\n\nsig Store {\n    fn read(): String\n}\n\npub fn run(): String {\n    return io::read()\n}\n")
            .unwrap();

        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(b"use vttasks::run\n\nfn main(): String {\n    return run()\n}\n")
            .unwrap();

        Compiler::new()
            .compile_file(main_path.to_str().unwrap())
            .expect("compile_file failed");
    }

    #[test]
    fn test_vtable_driven_by_explicit_binding() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let impl_path = dir.join("eb_impl.sa");
        let tasks_path = dir.join("eb_tasks.sa");
        let main_path = dir.join("eb_main.sa");

        std::fs::File::create(&impl_path)
            .unwrap()
            .write_all(b"pub fn read(): String {\n    return \"from-impl\"\n}\n")
            .unwrap();

        std::fs::File::create(&tasks_path)
            .unwrap()
            .write_all(b"mod eb_tasks(io: eb_impl::Store)\n\nsig Store {\n    fn read(): String\n}\n\npub fn run(): String {\n    return io::read()\n}\n")
            .unwrap();

        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(b"mod io: eb_impl::Store = eb_impl\nmod eb_tasks(io)\nuse eb_tasks::run\n\nfn main(): String {\n    return run()\n}\n")
            .unwrap();

        Compiler::new()
            .compile_file(main_path.to_str().unwrap())
            .expect("compile_file failed");
    }

    #[tokio::test]
    async fn test_vtable_substitution_end_to_end() {
        use crate::cli::config::ProgramSource;
        use crate::runtime::Runtime;
        use std::io::Write;

        let dir = std::env::temp_dir();
        let real_path = dir.join("sub_real.sa");
        let mock_path = dir.join("sub_mock.sa");
        let tasks_path = dir.join("sub_tasks.sa");
        let main_path = dir.join("sub_main.sa");

        std::fs::File::create(&real_path)
            .unwrap()
            .write_all(b"pub fn read(): String {\n    return \"real\"\n}\n")
            .unwrap();

        std::fs::File::create(&mock_path)
            .unwrap()
            .write_all(b"pub fn read(): String {\n    return \"mock\"\n}\n")
            .unwrap();

        std::fs::File::create(&tasks_path)
            .unwrap()
            .write_all(b"mod sub_tasks(io: sub_real::Store)\n\nsig Store {\n    fn read(): String\n}\n\npub fn run(): String {\n    return io::read()\n}\n")
            .unwrap();

        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(b"mod io: sub_real::Store = sub_mock\nmod sub_tasks(io)\nuse sub_tasks::run\n\nfn main(): String {\n    return run()\n}\n")
            .unwrap();

        let result = Runtime::builder(ProgramSource::File(main_path.to_str().unwrap().to_string()))
            .build()
            .run()
            .await
            .expect("runtime execution failed");

        assert_eq!(result.as_string().unwrap(), "mock");
    }

    #[tokio::test]
    async fn test_vtable_dispatch_end_to_end() {
        use crate::cli::config::ProgramSource;
        use crate::runtime::Runtime;
        use std::io::Write;

        let dir = std::env::temp_dir();
        let storage_path = dir.join("e2e_vtstore.sa");
        let tasks_path = dir.join("e2e_vttasks.sa");
        let main_path = dir.join("e2e_vtmain.sa");

        std::fs::File::create(&storage_path)
            .unwrap()
            .write_all(b"pub fn read(): String {\n    return \"from-storage\"\n}\n")
            .unwrap();

        std::fs::File::create(&tasks_path)
            .unwrap()
            .write_all(b"mod e2e_vttasks(io: e2e_vtstore::Store)\n\nsig Store {\n    fn read(): String\n}\n\npub fn run(): String {\n    return io::read()\n}\n")
            .unwrap();

        std::fs::File::create(&main_path)
            .unwrap()
            .write_all(b"use e2e_vttasks::run\n\nfn main(): String {\n    return run()\n}\n")
            .unwrap();

        let result = Runtime::builder(ProgramSource::File(main_path.to_str().unwrap().to_string()))
            .build()
            .run()
            .await
            .expect("runtime execution failed");

        assert_eq!(result.as_string().unwrap(), "from-storage");
    }
}
