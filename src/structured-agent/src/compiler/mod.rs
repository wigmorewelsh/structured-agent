pub mod parser;

use crate::analysis::{
    AnalysisRunner, ConstantConditionAnalyzer, DuplicateInjectionAnalyzer, EmptyBlockAnalyzer,
    EmptyFunctionAnalyzer, InfiniteLoopAnalyzer, OverwrittenValueAnalyzer,
    PlaceholderOveruseAnalyzer, ReachabilityAnalyzer, RedundantSelectAnalyzer,
    UnusedReturnValueAnalyzer, UnusedVariableAnalyzer, VariableShadowingAnalyzer,
};
use crate::ast::{self, Definition, Module};
use crate::diagnostics::{DiagnosticManager, DiagnosticReporter};
use crate::expressions::{
    AssignmentExpr, BooleanLiteralExpr, CallExpr, FunctionExpr, IfElseExpr, IfExpr, InjectionExpr,
    ListLiteralExpr, PlaceholderExpr, ReturnExpr, SelectClauseExpr, SelectExpr, StringLiteralExpr,
    UnitLiteralExpr, VariableAssignmentExpr, VariableExpr, WhileExpr,
};
use crate::typecheck::type_check_module;
use crate::types::{Expression, ExternalFunctionDefinition, FileId, Parameter, Type};

use combine::Parser as CombineParser;
use combine::stream::{easy, position};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tracing::{debug, error, info, warn};

fn convert_ast_type_to_type(ast_type: &ast::Type) -> Type {
    match ast_type {
        ast::Type::Unit => Type::unit(),
        ast::Type::Boolean => Type::boolean(),
        ast::Type::String => Type::string(),
        ast::Type::List(inner) => Type::list(convert_ast_type_to_type(inner)),
        ast::Type::Option(inner) => Type::option(convert_ast_type_to_type(inner)),
    }
}

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

pub trait Parser {
    fn parse(
        &self,
        program: &CompilationUnit,
        file_id: FileId,
        diagnostic_reporter: &DiagnosticReporter,
    ) -> Result<Module, String>;
}

pub struct CodespanParser {
    diagnostic_reporter: DiagnosticReporter,
}

impl CodespanParser {
    pub fn new(diagnostic_reporter: DiagnosticReporter) -> Self {
        Self {
            diagnostic_reporter,
        }
    }
}

impl Parser for CodespanParser {
    fn parse(
        &self,
        program: &CompilationUnit,
        file_id: FileId,
        _diagnostic_reporter: &DiagnosticReporter,
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

                if let Err(io_err) = self.diagnostic_reporter.emit_parse_error(
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

pub trait CompilerTrait {
    fn compile_program(&self, program: &CompilationUnit) -> Result<CompiledProgram, String>;
    fn compile_function(&self, function: &ast::Function) -> Result<FunctionExpr, String>;
}

#[derive(Debug)]
pub struct CompiledProgram {
    functions: HashMap<String, FunctionExpr>,
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

    pub fn add_function(&mut self, function: FunctionExpr) {
        let name = function.name.clone();
        if name == "main" {
            self.main_function = Some(name.clone());
        }
        self.functions.insert(name, function);
    }

    pub fn add_external_function(&mut self, external_function: ExternalFunctionDefinition) {
        let name = external_function.name.clone();
        self.external_functions.insert(name, external_function);
    }

    pub fn main_function(&self) -> Option<&FunctionExpr> {
        self.main_function
            .as_ref()
            .and_then(|name| self.functions.get(name))
    }

    pub fn functions(&self) -> &HashMap<String, FunctionExpr> {
        &self.functions
    }

    pub fn external_functions(&self) -> &HashMap<String, ExternalFunctionDefinition> {
        &self.external_functions
    }
}

pub struct Compiler {
    parser: Rc<dyn Parser>,
    diagnostic_manager: RefCell<DiagnosticManager>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        let diagnostic_manager = DiagnosticManager::new();
        let reporter = DiagnosticReporter::new(diagnostic_manager.files().clone());
        let parser = Rc::new(CodespanParser::new(reporter));
        Self {
            parser,
            diagnostic_manager: RefCell::new(diagnostic_manager),
        }
    }
}

impl CompilerTrait for Compiler {
    fn compile_program(&self, program: &CompilationUnit) -> Result<CompiledProgram, String> {
        debug!("Compiling program: {}", program.name());
        debug!("Source length: {} bytes", program.source().len());

        let mut diagnostic_manager = self.diagnostic_manager.borrow_mut();
        let file_id =
            diagnostic_manager.add_file(program.name().to_string(), program.source().to_string());

        let reporter = diagnostic_manager.reporter().clone();
        drop(diagnostic_manager);

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
            .with_analyzer(Box::new(UnusedReturnValueAnalyzer::new()));

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
                    match self.compile_function(&ast_function) {
                        Ok(compiled_function) => {
                            compiled_program.add_function(compiled_function);
                        }
                        Err(e) => {
                            error!("Failed to compile function {}: {}", ast_function.name, e);
                            return Err(e);
                        }
                    }
                }
                Definition::ExternalFunction(ast_external_function) => {
                    debug!(
                        "Compiling external function: {}",
                        ast_external_function.name
                    );
                    match Self::compile_external_function(&ast_external_function) {
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

    fn compile_function(&self, ast_func: &ast::Function) -> Result<FunctionExpr, String> {
        let mut compiled_statements = Vec::new();

        for stmt in &ast_func.body.statements {
            let compiled_stmt = Self::compile_statement(stmt)?;
            compiled_statements.push(compiled_stmt);
        }

        Ok(FunctionExpr {
            name: ast_func.name.clone(),
            parameters: ast_func
                .parameters
                .iter()
                .map(|p| Parameter::new(p.name.clone(), convert_ast_type_to_type(&p.param_type)))
                .collect(),
            return_type: convert_ast_type_to_type(&ast_func.return_type),
            body: compiled_statements,
            documentation: ast_func.documentation.clone(),
        })
    }
}

impl Compiler {
    pub fn compile_function(ast_func: &ast::Function) -> Result<FunctionExpr, String> {
        let mut compiled_statements = Vec::new();

        for stmt in &ast_func.body.statements {
            let compiled_stmt = Self::compile_statement(stmt)?;
            compiled_statements.push(compiled_stmt);
        }

        Ok(FunctionExpr {
            name: ast_func.name.clone(),
            parameters: ast_func
                .parameters
                .iter()
                .map(|p| Parameter::new(p.name.clone(), convert_ast_type_to_type(&p.param_type)))
                .collect(),
            return_type: convert_ast_type_to_type(&ast_func.return_type),
            body: compiled_statements,
            documentation: ast_func.documentation.clone(),
        })
    }

    pub fn compile_expression(ast_expr: &ast::Expression) -> Result<Box<dyn Expression>, String> {
        match ast_expr {
            ast::Expression::Call {
                function,
                arguments,
                ..
            } => {
                let compiled_args = arguments
                    .iter()
                    .map(|arg| Self::compile_expression(arg))
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(Box::new(CallExpr {
                    function: function.clone(),
                    arguments: compiled_args,
                }))
            }
            ast::Expression::Placeholder { .. } => Ok(Box::new(PlaceholderExpr {})),
            ast::Expression::Select(select_expression) => {
                let compiled_clauses = select_expression
                    .clauses
                    .iter()
                    .map(|clause| {
                        let expression_to_run =
                            Self::compile_expression(&clause.expression_to_run)?;
                        let expression_next = Self::compile_expression(&clause.expression_next)?;
                        Ok(SelectClauseExpr {
                            expression_to_run,
                            result_variable: clause.result_variable.clone(),
                            expression_next,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(Box::new(SelectExpr {
                    clauses: compiled_clauses,
                }))
            }
            ast::Expression::Variable { name, .. } => {
                Ok(Box::new(VariableExpr { name: name.clone() }))
            }
            ast::Expression::StringLiteral { value, .. } => Ok(Box::new(StringLiteralExpr {
                value: value.clone(),
            })),
            ast::Expression::BooleanLiteral { value, .. } => {
                Ok(Box::new(BooleanLiteralExpr { value: *value }))
            }
            ast::Expression::UnitLiteral { .. } => Ok(Box::new(UnitLiteralExpr {})),
            ast::Expression::ListLiteral { elements, .. } => {
                if elements.is_empty() {
                    return Err("Cannot infer type of empty list literal".to_string());
                }

                let compiled_elements: Vec<Box<dyn Expression>> = elements
                    .iter()
                    .map(|elem| Self::compile_expression(elem))
                    .collect::<Result<Vec<_>, String>>()?;

                let element_type = compiled_elements[0].return_type();

                for elem in &compiled_elements {
                    if elem.return_type() != element_type {
                        return Err("All list elements must have the same type".to_string());
                    }
                }

                Ok(Box::new(ListLiteralExpr {
                    elements: compiled_elements,
                    element_type,
                }))
            }
            ast::Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let compiled_condition = Self::compile_expression(condition)?;
                let compiled_then = Self::compile_expression(then_expr)?;
                let compiled_else = Self::compile_expression(else_expr)?;

                Ok(Box::new(IfElseExpr {
                    condition: compiled_condition,
                    then_expr: compiled_then,
                    else_expr: compiled_else,
                }))
            }
        }
    }

    pub fn compile_statement(ast_stmt: &ast::Statement) -> Result<Box<dyn Expression>, String> {
        match ast_stmt {
            ast::Statement::Injection(expr) => {
                let compiled_inner = Self::compile_expression(expr)?;
                Ok(Box::new(InjectionExpr {
                    inner: compiled_inner,
                }))
            }
            ast::Statement::Assignment {
                variable,
                expression,
                ..
            } => {
                let compiled_expression = Self::compile_expression(expression)?;
                Ok(Box::new(AssignmentExpr {
                    variable: variable.clone(),
                    expression: compiled_expression,
                }))
            }
            ast::Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => {
                let compiled_expression = Self::compile_expression(expression)?;
                Ok(Box::new(VariableAssignmentExpr {
                    variable: variable.clone(),
                    expression: compiled_expression,
                }))
            }
            ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                let compiled_condition = Self::compile_expression(condition)?;
                let compiled_body = body
                    .iter()
                    .map(|stmt| Self::compile_statement(stmt))
                    .collect::<Result<Vec<_>, String>>()?;
                let compiled_else = match else_body {
                    Some(else_stmts) => Some(
                        else_stmts
                            .iter()
                            .map(|stmt| Self::compile_statement(stmt))
                            .collect::<Result<Vec<_>, String>>()?,
                    ),
                    None => None,
                };
                Ok(Box::new(IfExpr {
                    condition: compiled_condition,
                    body: compiled_body,
                    else_body: compiled_else,
                }))
            }
            ast::Statement::While {
                condition, body, ..
            } => {
                let compiled_condition = Self::compile_expression(condition)?;

                let mut compiled_body = Vec::new();
                for stmt in body.iter() {
                    let compiled_stmt = Self::compile_statement(stmt)?;
                    compiled_body.push(compiled_stmt);
                }

                Ok(Box::new(WhileExpr {
                    condition: compiled_condition,
                    body: compiled_body,
                }))
            }
            ast::Statement::Return(expr) => {
                let compiled_expression = Self::compile_expression(expr)?;
                Ok(Box::new(ReturnExpr::new(compiled_expression)))
            }
            ast::Statement::ExpressionStatement(expr) => Self::compile_expression(expr),
        }
    }

    pub fn compile_external_function(
        ast_ext_func: &ast::ExternalFunction,
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
}

#[cfg(test)]
mod tests {
    use super::{CompilationUnit, Compiler, CompilerTrait};
    use crate::ast::{Expression as AstExpression, Statement as AstStatement};
    use crate::runtime::{Context, ExpressionValue, Runtime};
    use std::rc::Rc;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_compile_string_literal() {
        let ast_expr = AstExpression::StringLiteral {
            value: "Hello".to_string(),
            span: crate::types::Span::dummy(),
        };
        let compiled = Compiler::compile_expression(&ast_expr).unwrap();

        let dummy_program = CompilationUnit::from_string("fn main(): () {}".to_string());
        let runtime = Rc::new(Runtime::builder(dummy_program).build());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = compiled.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_compile_injection() {
        let ast_expr = AstExpression::StringLiteral {
            value: "Test injection".to_string(),
            span: crate::types::Span::dummy(),
        };
        let ast_stmt = AstStatement::Injection(ast_expr);
        let compiled = Compiler::compile_statement(&ast_stmt).unwrap();

        let dummy_program = CompilationUnit::from_string("fn main(): () {}".to_string());
        let runtime = Rc::new(Runtime::builder(dummy_program).build());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = compiled.evaluate(context.clone()).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "Test injection"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events_count(), 1);
        assert_eq!(context.get_event(0).unwrap().message, "Test injection");
    }

    #[tokio::test]
    async fn test_compile_variable() {
        let ast_expr = AstExpression::Variable {
            name: "test_var".to_string(),
            span: crate::types::Span::dummy(),
        };
        let compiled = Compiler::compile_expression(&ast_expr).unwrap();

        let dummy_program = CompilationUnit::from_string("fn main(): () {}".to_string());
        let runtime = Rc::new(Runtime::builder(dummy_program).build());
        let context = Arc::new(Context::with_runtime(runtime));
        context.declare_variable(
            "test_var".to_string(),
            ExpressionValue::String("variable_value".to_string()),
        );

        let result = compiled.evaluate(context).await.unwrap();

        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "variable_value"),
            _ => panic!("Expected string result"),
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

        let program = CompilationUnit::from_string(program_source.to_string());
        let compiler = Compiler::new();
        let compiled_program = compiler.compile_program(&program).unwrap();

        assert_eq!(compiled_program.functions().len(), 2);
        assert!(compiled_program.main_function().is_some());

        let runtime = Runtime::builder(program).build();
        let result = runtime.run().await.unwrap();

        match result {
            ExpressionValue::String(s) => assert_eq!(s, "Test completed"),
            _ => panic!("Expected string result"),
        }
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

        let program = CompilationUnit::from_string(program_source.to_string());
        let compiler = Compiler::new();
        let compiled_program = compiler.compile_program(&program).unwrap();

        assert_eq!(compiled_program.functions().len(), 4);
        assert!(compiled_program.main_function().is_some());

        let runtime = Runtime::builder(program).build();
        let result = runtime.run().await.unwrap();

        match result {
            ExpressionValue::String(s) => assert_eq!(s, "<result>\n## calculator\n</result>"),
            _ => panic!("Expected string result, got: {:?}", result),
        }
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

        let program = CompilationUnit::from_string(program_source.to_string());
        let compiler = Compiler::new();
        let compiled_program = compiler.compile_program(&program).unwrap();

        assert_eq!(compiled_program.functions().len(), 2);
        assert!(compiled_program.main_function().is_some());

        let runtime = Runtime::builder(program).build();
        let result = runtime.run().await.unwrap();

        match result {
            ExpressionValue::String(s) => assert_eq!(s, "<message>\nSystem ready\n</message>"),
            _ => panic!("Expected string result, got: {:?}", result),
        }
    }
}
