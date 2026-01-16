use std::{any::Any, collections::HashMap, rc::Rc};

use crate::expressions::FunctionExpr;

#[derive(Debug, Clone)]
pub struct Event {
    pub message: String,
}

pub struct Context {
    pub parent: Option<Rc<Context>>,
    pub events: Vec<Event>,
    pub variables: HashMap<String, ExprResult>,
    pub registry: FunctionRegistry,
    pub engine: Rc<dyn LanguageEngine>,
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            parent: None,
            events: Vec::new(),
            variables: HashMap::new(),
            registry: FunctionRegistry::new(),
            engine: Rc::new(PrintEngine {}),
        }
    }

    pub fn with_parent(parent: Rc<Context>) -> Self {
        Self {
            parent: Some(parent),
            events: Vec::new(),
            variables: HashMap::new(),
            registry: FunctionRegistry::new(),
            engine: Rc::new(PrintEngine {}),
        }
    }

    pub fn with_engine(engine: Rc<dyn LanguageEngine>) -> Self {
        Self {
            parent: None,
            events: Vec::new(),
            variables: HashMap::new(),
            registry: FunctionRegistry::new(),
            engine,
        }
    }

    pub fn add_event(&mut self, message: String) {
        self.events.push(Event { message });
    }

    pub fn get_variable(&self, name: &str) -> Option<&ExprResult> {
        self.variables
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_variable(name)))
    }

    pub fn set_variable(&mut self, name: String, value: ExprResult) {
        self.variables.insert(name, value);
    }

    pub fn create_child(&self) -> Self {
        Self {
            parent: Some(Rc::new(Context {
                parent: self.parent.clone(),
                events: self.events.clone(),
                variables: self.variables.clone(),
                registry: self.registry.clone(),
                engine: self.engine.clone(),
            })),
            events: Vec::new(),
            variables: HashMap::new(),
            registry: self.registry.clone(),
            engine: self.engine.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub name: String,
}

impl Type {
    pub fn string() -> Self {
        Self {
            name: "String".to_string(),
        }
    }

    pub fn unit() -> Self {
        Self {
            name: "()".to_string(),
        }
    }
}

pub trait Expression: std::fmt::Debug {
    fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String>;
    fn return_type(&self) -> Type;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Expression>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprResult {
    String(String),
    Unit,
}

impl ExprResult {
    pub fn as_string(&self) -> Result<&str, String> {
        match self {
            ExprResult::String(s) => Ok(s),
            _ => Err("Expected string result".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionRegistry {
    functions: HashMap<String, FunctionExpr>,
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    pub fn register_function(&mut self, info: FunctionExpr) {
        self.functions.insert(info.name.clone(), info);
    }

    pub fn get_function(&self, name: &str) -> Option<&FunctionExpr> {
        self.functions.get(name)
    }

    pub fn list_functions(&self) -> Vec<&str> {
        self.functions.keys().map(|s| s.as_str()).collect()
    }
}

pub trait LanguageEngine {
    fn untyped(&self, context: &Context) -> String;
}

pub struct PrintEngine {}

impl LanguageEngine for PrintEngine {
    fn untyped(&self, _context: &Context) -> String {
        println!("PrintEngine {{}}");
        format!("PrintEngine {{}}")
    }
}
