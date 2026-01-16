use crate::runtime::Runtime;
use std::{collections::HashMap, rc::Rc};

#[derive(Debug, Clone)]
pub struct Event {
    pub message: String,
}

pub struct Context {
    pub parent: Option<Rc<Context>>,
    pub events: Vec<Event>,
    pub variables: HashMap<String, ExprResult>,
    runtime: Rc<Runtime>,
}

impl Context {
    pub fn with_runtime(runtime: Rc<Runtime>) -> Self {
        Self {
            parent: None,
            events: Vec::new(),
            variables: HashMap::new(),
            runtime,
        }
    }

    pub fn with_parent(parent: Rc<Context>, runtime: Rc<Runtime>) -> Self {
        Self {
            parent: Some(parent),
            events: Vec::new(),
            variables: HashMap::new(),
            runtime,
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
                runtime: self.runtime.clone(),
            })),
            events: Vec::new(),
            variables: HashMap::new(),
            runtime: self.runtime.clone(),
        }
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
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
