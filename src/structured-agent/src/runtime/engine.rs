use crate::expressions::FunctionExpr;
use crate::runtime::{Context, ExprResult};
use crate::types::LanguageEngine;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Runtime {
    function_registry: HashMap<String, FunctionExpr>,
    language_engine: Rc<dyn LanguageEngine>,
}

#[derive(Debug)]
pub enum RuntimeError {
    FunctionNotFound(String),
    InvalidArguments(String),
    ExecutionError(String),
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            function_registry: HashMap::new(),
            language_engine: Rc::new(crate::types::PrintEngine {}),
        }
    }

    pub fn with_engine(engine: Rc<dyn LanguageEngine>) -> Self {
        Self {
            function_registry: HashMap::new(),
            language_engine: engine,
        }
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
        &*self.language_engine
    }

    pub fn run(&self, program: &dyn crate::types::Expression) -> Result<ExprResult, RuntimeError> {
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
        }
    }
}
