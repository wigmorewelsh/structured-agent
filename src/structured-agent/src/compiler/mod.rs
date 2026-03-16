pub(crate) mod discovery;
pub mod parser;
pub(crate) mod sigs;

use crate::analysis::{
    AnalysisRunner, ConstantConditionAnalyzer, DuplicateInjectionAnalyzer, EmptyBlockAnalyzer,
    EmptyFunctionAnalyzer, InfiniteLoopAnalyzer, OverwrittenValueAnalyzer,
    PlaceholderOveruseAnalyzer, ReachabilityAnalyzer, RedundantSelectAnalyzer,
    UnusedExpressionAnalyzer, UnusedReturnValueAnalyzer, UnusedVariableAnalyzer,
    VariableShadowingAnalyzer,
};
use crate::ast::{Definition, Module, SigFunction};
use crate::bytecode::BytecodeCompiler;
use crate::diagnostics::{DiagnosticManager, DiagnosticReporter};
use crate::typecheck::TypeChecker;
use crate::typecheck::checker::{FunctionSignatureTuple, ModuleVisibility};
use crate::types::{
    ExecutableFunction, ExternalFunctionDefinition, FileId, Function, Parameter, Type,
};

use combine::Parser as CombineParser;
use combine::stream::{easy, position};
use discovery::{Discoverer, FileDiscoverer, InMemoryDiscoverer, discover};
use sigs::{SigTable, collect_sigs, sigs_visible_to_module};
use std::collections::HashMap;

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

#[derive(Debug)]
pub struct CompiledProgram {
    functions: HashMap<String, Box<dyn ExecutableFunction>>,
    external_functions: HashMap<String, ExternalFunctionDefinition>,
    struct_definitions: HashMap<String, Vec<(String, Type)>>,
    sig_definitions: HashMap<String, Vec<SigFunction>>,
    pub module_visibility: ModuleVisibility,
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
            main_function: None,
            source_path: None,
        }
    }

    pub fn with_source_path(mut self, path: Option<String>) -> Self {
        self.source_path = path;
        self
    }

    pub fn source_path(&self) -> Option<&str> {
        self.source_path.as_deref()
    }

    pub fn add_function(&mut self, function: Box<dyn ExecutableFunction>) {
        let name = Function::name(function.as_ref()).to_string();
        if name == "main" {
            self.main_function = Some(name.clone());
        }
        self.functions.insert(name, function);
    }

    pub fn add_external_function(&mut self, external_function: ExternalFunctionDefinition) {
        self.external_functions
            .insert(external_function.name.clone(), external_function);
    }

    pub fn main_function(&self) -> Option<&Box<dyn ExecutableFunction>> {
        self.main_function
            .as_ref()
            .and_then(|name| self.functions.get(name))
    }

    pub fn functions(&self) -> &HashMap<String, Box<dyn ExecutableFunction>> {
        &self.functions
    }

    pub fn external_functions(&self) -> &HashMap<String, ExternalFunctionDefinition> {
        &self.external_functions
    }

    pub fn struct_definitions(&self) -> &HashMap<String, Vec<(String, Type)>> {
        &self.struct_definitions
    }

    pub fn add_struct_definition(&mut self, name: String, fields: Vec<(String, Type)>) {
        self.struct_definitions.insert(name, fields);
    }

    pub fn add_sig_definition(&mut self, name: String, functions: Vec<SigFunction>) {
        self.sig_definitions.insert(name, functions);
    }

    pub fn sig_definitions(&self) -> &HashMap<String, Vec<SigFunction>> {
        &self.sig_definitions
    }

    pub fn module_visibility(&self) -> &ModuleVisibility {
        &self.module_visibility
    }
}

pub struct Compiler {
    parser: CodespanParser,
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
        }
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

        let modules = discover(entry_path, entry_source, discoverer, |path, source| {
            let unit = CompilationUnit::from_file(path.to_string(), source.to_string());
            let file_id = diagnostics.add_file(path.to_string(), source.to_string());
            let reporter = diagnostics.reporter().clone();
            parser
                .parse(&unit, file_id, &reporter)
                .map(|m| (file_id, m))
        })?;

        let sig_table: SigTable = collect_sigs(&modules);

        for parsed in &modules {
            let reporter = diagnostics.reporter().clone();
            check_module(parsed, &sig_table, &reporter)
                .map_err(|e| format!("In {}: {}", parsed.name, e))?;
        }

        let mut compiled = CompiledProgram {
            module_visibility: sig_table.visibility.clone(),
            ..CompiledProgram::new().with_source_path(source_path)
        };

        for parsed in &modules {
            let prefix = (!parsed.is_entry).then_some(parsed.name.as_str());
            emit_definitions(&parsed.module, prefix, &mut compiled)?;
        }

        Ok(compiled)
    }
}

fn check_module(
    parsed: &discovery::ParsedModule,
    sig_table: &SigTable,
    reporter: &DiagnosticReporter,
) -> Result<(), String> {
    let external_sigs = if parsed.is_entry {
        sigs_visible_to_module(&parsed.module, sig_table)
    } else {
        sig_table.external_sigs.clone()
    };
    let mut checker = TypeChecker::new();
    checker
        .check_module_with_external_sigs(
            &parsed.module,
            parsed.file_id,
            &external_sigs,
            &sig_table.visibility,
        )
        .map_err(|e| {
            error!("Type checking failed: {}", e);
            if let Err(io_err) = reporter.emit_type_error(&e) {
                eprintln!("Failed to emit type error: {}", io_err);
            }
            format!("Type error: {}", e)
        })?;

    let warnings = build_analysis_runner().run(&parsed.module, parsed.file_id);
    if !warnings.is_empty() {
        warn!("Analysis found {} warnings", warnings.len());
    }
    for warning in &warnings {
        if let Err(io_err) = reporter.emit_diagnostic(&warning.to_diagnostic()) {
            eprintln!("Failed to emit warning: {}", io_err);
        }
    }

    Ok(())
}

fn emit_definitions(
    module: &Module,
    prefix: Option<&str>,
    compiled: &mut CompiledProgram,
) -> Result<(), String> {
    for definition in &module.definitions {
        match definition {
            Definition::Function(f) => {
                let mut f = f.clone();
                if let Some(p) = prefix {
                    f.name = format!("{}.{}", p, f.name);
                }
                debug!("Emitting function: {}", f.name);
                compiled.add_function(BytecodeCompiler::compile_function(&f)?);
            }
            Definition::ExternalFunction(f) => {
                let mut f = f.clone();
                if let Some(p) = prefix {
                    f.name = format!("{}.{}", p, f.name);
                }
                debug!("Emitting external function: {}", f.name);
                compiled.add_external_function(compile_external_function(&f)?);
            }
            Definition::Struct(s) => {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), ast_type_to_type(&f.field_type)))
                    .collect();
                compiled.add_struct_definition(s.name.clone(), fields);
            }
            Definition::Signature {
                name, functions, ..
            } => {
                compiled.add_sig_definition(name.clone(), functions.clone());
            }
            Definition::Use { .. } | Definition::ModuleHeader { .. } => {}
        }
    }
    Ok(())
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
    }
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
                b"use greetlib.greet\n\nfn main(): String {\n    return greet(\"world\")\n}\n",
            )
            .unwrap();

        let compiled = Compiler::new()
            .compile_file(main_path.to_str().unwrap())
            .expect("compile_file failed");

        assert!(compiled.functions().contains_key("main"));
        assert!(compiled.functions().contains_key("greetlib.greet"));
        assert!(compiled.functions().contains_key("greetlib.internal"));
        assert_eq!(
            compiled.module_visibility().get("greetlib.greet"),
            Some(&true)
        );
        assert_eq!(
            compiled.module_visibility().get("greetlib.internal"),
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
            .write_all(b"use vislib.greet\n\nfn main(): String {\n    return greet(\"world\")\n}\n")
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
            .write_all(b"use privlib.secret\n\nfn main(): String {\n    return secret(\"x\")\n}\n")
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
}
