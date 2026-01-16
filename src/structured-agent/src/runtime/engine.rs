use crate::compiler::{CompilationUnit, Compiler, CompilerTrait};
use crate::expressions::FunctionExpr;
use crate::runtime::{Context, ExprResult};
use crate::types::LanguageEngine;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Runtime {
    function_registry: HashMap<String, FunctionExpr>,
    language_engine: Rc<dyn LanguageEngine>,
    compiler: Rc<dyn CompilerTrait>,
}

pub struct RuntimeBuilder {
    function_registry: HashMap<String, FunctionExpr>,
    language_engine: Option<Rc<dyn LanguageEngine>>,
    compiler: Option<Rc<dyn CompilerTrait>>,
}

#[derive(Debug)]
pub enum RuntimeError {
    FunctionNotFound(String),
    InvalidArguments(String),
    ExecutionError(String),
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            function_registry: HashMap::new(),
            language_engine: None,
            compiler: None,
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

    pub fn build(self) -> Runtime {
        Runtime {
            function_registry: self.function_registry,
            language_engine: self
                .language_engine
                .unwrap_or_else(|| Rc::new(crate::types::PrintEngine {})),
            compiler: self.compiler.unwrap_or_else(|| Rc::new(Compiler::new())),
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
            .insert(function.name.clone(), function);
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionExpr> {
        self.function_registry.get(name)
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

    pub fn run(&self, program_source: &str) -> Result<ExprResult, RuntimeError> {
        let program = CompilationUnit::from_string(program_source.to_string());
        let compiled_program = self
            .compiler
            .compile_program(&program)
            .map_err(RuntimeError::ExecutionError)?;

        // Create a new runtime with all compiled functions registered
        let mut runtime_with_functions = self.clone();
        for (_, function) in compiled_program.functions() {
            runtime_with_functions.register_function(function.clone());
        }

        if let Some(main_function) = compiled_program.main_function() {
            runtime_with_functions.run_expression(main_function)
        } else {
            Err(RuntimeError::FunctionNotFound("main".to_string()))
        }
    }

    pub fn run_expression(
        &self,
        program: &dyn crate::types::Expression,
    ) -> Result<ExprResult, RuntimeError> {
        let mut context = Context::with_runtime(Rc::new(self.clone()));
        program
            .evaluate(&mut context)
            .map_err(RuntimeError::ExecutionError)
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
            language_engine: self.language_engine.clone(),
            compiler: self.compiler.clone(),
        }
    }
}
