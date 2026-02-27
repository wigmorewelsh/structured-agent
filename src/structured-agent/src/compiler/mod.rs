pub mod parser;

use crate::analysis::{
    AnalysisRunner, ConstantConditionAnalyzer, DuplicateInjectionAnalyzer, EmptyBlockAnalyzer,
    EmptyFunctionAnalyzer, InfiniteLoopAnalyzer, OverwrittenValueAnalyzer,
    PlaceholderOveruseAnalyzer, ReachabilityAnalyzer, RedundantSelectAnalyzer,
    UnusedExpressionAnalyzer, UnusedReturnValueAnalyzer, UnusedVariableAnalyzer,
    VariableShadowingAnalyzer,
};
use crate::ast::{Definition, Module};
use crate::diagnostics::{DiagnosticManager, DiagnosticReporter};
use crate::typecheck::type_check_module;
use crate::types::{ExecutableFunction, ExternalFunctionDefinition, FileId, Function};

use combine::Parser as CombineParser;
use combine::stream::{easy, position};
use std::collections::HashMap;
use tracing::{debug, error, warn};

use crate::bytecode::BytecodeCompiler;
use crate::types::{Parameter, Type};

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

pub struct CodespanParser {}

impl CodespanParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl CodespanParser {
    pub fn parse(
        &self,
        program: &CompilationUnit,
        file_id: FileId,
        diagnostic_reporter: &DiagnosticReporter,
    ) -> Result<Module, String> {
        debug!("Parsing source code");
        let input = program.source();
        let stream = easy::Stream(position::Stream::with_positioner(
            input,
            position::IndexPositioner::new(),
        ));

        let result = parser::parse_program(file_id).parse(stream);

        match result {
            Ok((module, _)) => {
                debug!(
                    "Parser succeeded, found {} definitions",
                    module.definitions.len()
                );
                Ok(module)
            }
            Err(e) => {
                let error_str = format!("{}", e);
                let byte_offset = e.position;
                error!("Parser error at position {}: {}", byte_offset, error_str);

                let clean_message = error_str.lines().skip(1).collect::<Vec<_>>().join("\n");

                if let Err(io_err) = diagnostic_reporter.emit_parse_error(
                    file_id,
                    &clean_message,
                    Some((byte_offset, byte_offset + 1)),
                ) {
                    eprintln!("Failed to emit diagnostic: {}", io_err);
                }

                Err("Parse error".to_string())
            }
        }
    }
}

#[derive(Debug)]
pub struct CompiledProgram {
    functions: HashMap<String, Box<dyn ExecutableFunction>>,
    external_functions: HashMap<String, ExternalFunctionDefinition>,
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
        let name = external_function.name.clone();
        self.external_functions.insert(name, external_function);
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
}

pub fn compile_external_function(
    ast_ext_func: &crate::ast::ExternalFunction,
) -> Result<ExternalFunctionDefinition, String> {
    let parameters = ast_ext_func
        .parameters
        .iter()
        .map(|p| Parameter::new(p.name.clone(), convert_ast_type_to_type(&p.param_type)))
        .collect();

    Ok(ExternalFunctionDefinition::new(
        ast_ext_func.name.clone(),
        parameters,
        convert_ast_type_to_type(&ast_ext_func.return_type),
    ))
}

fn convert_ast_type_to_type(ast_type: &crate::ast::Type) -> Type {
    match ast_type {
        crate::ast::Type::Unit => Type::unit(),
        crate::ast::Type::Boolean => Type::boolean(),
        crate::ast::Type::String => Type::string(),
        crate::ast::Type::List(inner) => Type::list(convert_ast_type_to_type(inner)),
        crate::ast::Type::Option(inner) => Type::option(convert_ast_type_to_type(inner)),
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
        let parser = CodespanParser::new();
        Self { parser }
    }
}

impl Compiler {
    pub fn compile_program(&self, program: &CompilationUnit) -> Result<CompiledProgram, String> {
        debug!("Compiling program: {}", program.name());
        debug!("Source length: {} bytes", program.source().len());

        let mut diagnostic_manager = DiagnosticManager::new();
        let file_id =
            diagnostic_manager.add_file(program.name().to_string(), program.source().to_string());

        let reporter = diagnostic_manager.reporter().clone();

        debug!("Starting parser");
        let module = match self.parser.parse(program, file_id, &reporter) {
            Ok(m) => {
                debug!("Parsing completed successfully");
                debug!("Found {} definitions", m.definitions.len());
                m
            }
            Err(e) => {
                error!("Parsing failed: {}", e);
                return Err(e);
            }
        };

        debug!("Starting type checking");
        if let Err(type_error) = type_check_module(&module, file_id) {
            error!("Type checking failed: {}", type_error);
            if let Err(io_err) = reporter.emit_type_error(&type_error) {
                eprintln!("Failed to emit type error diagnostic: {}", io_err);
            }
            return Err(format!("Type error: {}", type_error));
        }
        debug!("Type checking completed successfully");

        let mut runner = AnalysisRunner::new()
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
            .with_analyzer(Box::new(UnusedExpressionAnalyzer::new()));

        debug!("Running analysis");
        let warnings = runner.run(&module, file_id);
        if !warnings.is_empty() {
            warn!("Analysis found {} warnings", warnings.len());
        }
        for warning in &warnings {
            debug!("Warning: {:?}", warning);
            if let Err(io_err) = reporter.emit_diagnostic(&warning.to_diagnostic()) {
                eprintln!("Failed to emit warning diagnostic: {}", io_err);
            }
        }

        let mut compiled_program =
            CompiledProgram::new().with_source_path(program.path().map(String::from));

        debug!("Compiling definitions");
        for definition in module.definitions {
            match definition {
                Definition::Function(ast_function) => {
                    debug!("Compiling function: {}", ast_function.name);
                    let func_expr = BytecodeCompiler::compile_function(&ast_function)?;
                    compiled_program.add_function(func_expr);
                }
                Definition::ExternalFunction(ast_external_function) => {
                    debug!(
                        "Compiling external function: {}",
                        ast_external_function.name
                    );
                    match compile_external_function(&ast_external_function) {
                        Ok(compiled_external_function) => {
                            compiled_program.add_external_function(compiled_external_function);
                        }
                        Err(e) => {
                            error!(
                                "Failed to compile external function {}: {}",
                                ast_external_function.name, e
                            );
                            return Err(e);
                        }
                    }
                }
            }
        }

        debug!("Compilation completed successfully");
        Ok(compiled_program)
    }
}

#[cfg(test)]
mod tests {
    use super::{CompilationUnit, Compiler};
    use crate::runtime::{ExpressionValue, Runtime};

    async fn run_test_with_compiler(program_source: &str, expected: &str) {
        let program = CompilationUnit::from_string(program_source.to_string());
        let runtime = Runtime::builder(program).build();
        let result = runtime.run().await.unwrap();

        match result {
            ExpressionValue::String(s) => assert_eq!(s, expected),
            _ => panic!("Expected string result, got: {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_new_architecture_end_to_end() {
        let program_source = r#"
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
"#;
        run_test_with_compiler(program_source, "Test completed").await;
    }

    #[tokio::test]
    async fn test_select_statement_end_to_end() {
        let program_source = r#"
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
"#;
        run_test_with_compiler(
            program_source,
            "<calculator>\n    <param name=\"x\">5</param>\n    <param name=\"y\">3</param>\n    <result>\n    ## calculator\n    </result>\n</calculator>",
        )
        .await;
    }

    #[test]
    fn test_control_flow_analysis_warnings() {
        let program_source = r#"
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

        let program = CompilationUnit::from_string(program_source.to_string());
        let compiler = Compiler::new();
        let result = compiler.compile_program(&program);

        assert!(result.is_ok());
        let compiled_program = result.unwrap();
        assert_eq!(compiled_program.functions().len(), 4);
    }

    #[tokio::test]
    async fn test_simple_function() {
        let program_source = r#"
fn add(a: String, b: String): String {
    return "result"
}

fn main(): String {
    return add("1", "2")
}
"#;
        run_test_with_compiler(program_source, "result").await;
    }

    #[tokio::test]
    async fn test_multi_function() {
        let program_source = r#"
fn greet(name: String): () {
    "Hello, "!
    name!
}

fn main(): String {
    greet("World")
    "Done"!
}
"#;
        run_test_with_compiler(program_source, "Done").await;
    }

    #[tokio::test]
    async fn test_unit_literal_end_to_end() {
        let source = r#"
fn main(): () {
    return ()
}
"#;
        let program = CompilationUnit::from_string(source.to_string());
        let runtime = Runtime::builder(program).build();
        let result = runtime.run().await.unwrap();

        assert_eq!(result, ExpressionValue::Unit);
    }

    #[tokio::test]
    async fn test_if_else_expression_end_to_end() {
        let program_source = r#"
fn choose_message(ready: Boolean): String {
    return if ready { "System ready" } else { "System not ready" }
}

fn main(): String {
    let message = choose_message(true)
    message!
}
"#;
        run_test_with_compiler(
            program_source,
            "<choose_message>\n    <param name=\"ready\">true</param>\n    <result>\n    System ready\n    </result>\n</choose_message>",
        )
        .await;
    }
}
