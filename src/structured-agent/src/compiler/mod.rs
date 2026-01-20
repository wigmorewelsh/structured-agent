pub mod parser;

use crate::ast;
use crate::expressions::{
    AssignmentExpr, BooleanLiteralExpr, CallExpr, FunctionExpr, IfExpr, InjectionExpr,
    PlaceholderExpr, SelectClauseExpr, SelectExpr, StringLiteralExpr, VariableExpr, WhileExpr,
};
use crate::types::{Expression, ExternalFunctionDefinition, Type};

use combine::stream::position::IndexPositioner;
use std::collections::HashMap;
use std::rc::Rc;

fn convert_ast_type_to_type(ast_type: &ast::Type) -> Type {
    match ast_type {
        ast::Type::Named(name) => Type { name: name.clone() },
        ast::Type::Unit => Type::unit(),
        ast::Type::Boolean => Type::boolean(),
    }
}

#[derive(Debug, Clone)]
pub struct CompilationUnit {
    source: String,
    name: String,
}

impl CompilationUnit {
    pub fn new(name: String, source: String) -> Self {
        Self { name, source }
    }

    pub fn from_string(source: String) -> Self {
        Self {
            name: "main".to_string(),
            source,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

pub trait Parser {
    fn parse(
        &self,
        program: &CompilationUnit,
    ) -> Result<(Vec<ast::Function>, Vec<ast::ExternalFunction>), String>;
}

pub struct DefaultParser;

impl Parser for DefaultParser {
    fn parse(
        &self,
        program: &CompilationUnit,
    ) -> Result<(Vec<ast::Function>, Vec<ast::ExternalFunction>), String> {
        use combine::{EasyParser, stream::position};

        let input = program.source();
        let stream = position::Stream::with_positioner(input, IndexPositioner::new());

        match parser::parse_program().easy_parse(stream) {
            Ok(((functions, external_functions), _)) => Ok((functions, external_functions)),
            Err(e) => {
                let formatted_error = format_parse_error_with_position(&e, input);
                Err(formatted_error)
            }
        }
    }
}

fn format_parse_error_with_position(
    error: &combine::easy::Errors<char, &str, usize>,
    source: &str,
) -> String {
    let position = error.position;
    let (line, column) = calculate_line_column(source, position);

    // Use combine's built-in Display but clean it up
    let error_text = error.to_string();
    let clean_error = clean_combine_error(&error_text);

    format!(
        "Parse error at line {}, column {}: {}",
        line, column, clean_error
    )
}

fn clean_combine_error(error_text: &str) -> String {
    // Parse combine's format: "Parse error at {pos}\nUnexpected `{token}`\nExpected `{token}`\n"
    let lines: Vec<&str> = error_text.lines().collect();
    let mut messages = Vec::new();

    for line in lines {
        if line.starts_with("Unexpected") {
            let clean = line.replace("Unexpected `", "unexpected ").replace("`", "");
            messages.push(clean);
        } else if line.starts_with("Expected") {
            let clean = line.replace("Expected `", "expected ").replace("`", "");
            messages.push(clean);
        } else if !line.starts_with("Parse error at") && !line.trim().is_empty() {
            messages.push(line.to_string());
        }
    }

    if messages.is_empty() {
        "syntax error".to_string()
    } else {
        messages.join(", ")
    }
}

fn calculate_line_column(source: &str, position: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;

    for (i, ch) in source.char_indices() {
        if i >= position {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
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
}

impl CompiledProgram {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            external_functions: HashMap::new(),
            main_function: None,
        }
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

    pub fn get_function(&self, name: &str) -> Option<&FunctionExpr> {
        self.functions.get(name)
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
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            parser: Rc::new(DefaultParser),
        }
    }

    pub fn with_parser(parser: Rc<dyn Parser>) -> Self {
        Self { parser }
    }
}

impl CompilerTrait for Compiler {
    fn compile_program(&self, program: &CompilationUnit) -> Result<CompiledProgram, String> {
        let (ast_functions, ast_external_functions) = self.parser.parse(program)?;
        let mut compiled_program = CompiledProgram::new();

        for ast_function in ast_functions {
            let compiled_function = self.compile_function(&ast_function)?;
            compiled_program.add_function(compiled_function);
        }

        for ast_external_function in ast_external_functions {
            let compiled_external_function =
                Self::compile_external_function(&ast_external_function)?;
            compiled_program.add_external_function(compiled_external_function);
        }

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
                .map(|p| (p.name.clone(), convert_ast_type_to_type(&p.param_type)))
                .collect(),
            return_type: convert_ast_type_to_type(&ast_func.return_type),
            body: compiled_statements,
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
                .map(|p| (p.name.clone(), convert_ast_type_to_type(&p.param_type)))
                .collect(),
            return_type: convert_ast_type_to_type(&ast_func.return_type),
            body: compiled_statements,
        })
    }

    pub fn compile_expression(ast_expr: &ast::Expression) -> Result<Box<dyn Expression>, String> {
        match ast_expr {
            ast::Expression::StringLiteral(value) => Ok(Box::new(StringLiteralExpr {
                value: value.clone(),
            })),
            ast::Expression::BooleanLiteral(value) => {
                Ok(Box::new(BooleanLiteralExpr { value: *value }))
            }
            ast::Expression::Variable(name) => Ok(Box::new(VariableExpr { name: name.clone() })),
            ast::Expression::Call {
                target,
                function,
                arguments,
                is_method,
            } => {
                let compiled_args = arguments
                    .iter()
                    .map(|arg| Self::compile_expression(arg))
                    .collect::<Result<Vec<_>, String>>()?;

                Ok(Box::new(CallExpr {
                    target: target.clone(),
                    function: function.clone(),
                    arguments: compiled_args,
                    is_method: *is_method,
                }))
            }
            ast::Expression::Placeholder => Ok(Box::new(PlaceholderExpr {})),
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
            } => {
                let compiled_expression = Self::compile_expression(expression)?;
                Ok(Box::new(AssignmentExpr {
                    variable: variable.clone(),
                    expression: compiled_expression,
                }))
            }
            ast::Statement::If { condition, body } => {
                let compiled_condition = Self::compile_expression(condition)?;
                let compiled_body = body
                    .iter()
                    .map(|stmt| Self::compile_statement(stmt))
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(Box::new(IfExpr {
                    condition: compiled_condition,
                    body: compiled_body,
                }))
            }
            ast::Statement::While { condition, body } => {
                let compiled_condition = Self::compile_expression(condition)?;
                let compiled_body = body
                    .iter()
                    .map(|stmt| Self::compile_statement(stmt))
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(Box::new(WhileExpr {
                    condition: compiled_condition,
                    body: compiled_body,
                }))
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
            .map(|p| (p.name.clone(), convert_ast_type_to_type(&p.param_type)))
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
    use crate::runtime::{Context, ExprResult, Runtime};
    use std::rc::Rc;

    #[tokio::test]
    async fn test_compile_string_literal() {
        let ast_expr = AstExpression::StringLiteral("Hello".to_string());
        let compiled = Compiler::compile_expression(&ast_expr).unwrap();

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = compiled.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_compile_injection() {
        let ast_expr = AstExpression::StringLiteral("Test injection".to_string());
        let ast_stmt = AstStatement::Injection(ast_expr);
        let compiled = Compiler::compile_statement(&ast_stmt).unwrap();

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = compiled.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Test injection"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(context.events.len(), 1);
        assert_eq!(context.events[0].message, "Test injection");
    }

    #[tokio::test]
    async fn test_compile_variable() {
        let ast_expr = AstExpression::Variable("test_var".to_string());
        let compiled = Compiler::compile_expression(&ast_expr).unwrap();

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        context.set_variable(
            "test_var".to_string(),
            ExprResult::String("variable_value".to_string()),
        );

        let result = compiled.evaluate(&mut context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "variable_value"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_new_architecture_end_to_end() {
        let program_source = r#"
fn greet(name: String) -> () {
    "Hello, "!
    name!
    "!"!
}

fn main() -> () {
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

        let runtime = Runtime::new();
        let result = runtime.run(program_source).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Test completed"),
            _ => panic!("Expected string result"),
        }
    }

    #[tokio::test]
    async fn test_select_statement_end_to_end() {
        let program_source = r#"
fn add(a: String, b: String) -> () {
    "Adding numbers"
}

fn subtract(a: String, b: String) -> () {
    "Subtracting numbers"
}

fn calculator(x: String, y: String) -> () {
    let result = select {
        add(x, y) as sum => sum,
        subtract(x, y) as diff => diff
    }
    result
}

fn main() -> () {
    let result = calculator("5", "3")
    result!
}
"#;

        let program = CompilationUnit::from_string(program_source.to_string());
        let compiler = Compiler::new();
        let compiled_program = compiler.compile_program(&program).unwrap();

        assert_eq!(compiled_program.functions().len(), 4);
        assert!(compiled_program.main_function().is_some());

        let runtime = Runtime::new();
        let result = runtime.run(program_source).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "Adding numbers"),
            _ => panic!("Expected string result"),
        }
    }
}
