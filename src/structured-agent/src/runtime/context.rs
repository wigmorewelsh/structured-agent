use crate::runtime::Runtime;
use arrow::array::ListArray;
use dashmap::DashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Event {
    pub message: String,
}

pub struct Context {
    pub parent: Option<Arc<Context>>,
    events: RefCell<Vec<Event>>,
    pub variables: DashMap<String, ExpressionValue>,
    pub is_scope_boundary: bool,
    return_value: RefCell<Option<ExpressionValue>>,
    runtime: Rc<Runtime>,
}

impl Context {
    pub fn with_runtime(runtime: Rc<Runtime>) -> Self {
        Self {
            parent: None,
            events: RefCell::new(Vec::new()),
            variables: DashMap::new(),
            is_scope_boundary: true,
            return_value: RefCell::new(None),
            runtime,
        }
    }

    pub fn with_parent(parent: Arc<Context>, runtime: Rc<Runtime>) -> Self {
        Self {
            parent: Some(parent),
            events: RefCell::new(Vec::new()),
            variables: DashMap::new(),
            is_scope_boundary: false,
            return_value: RefCell::new(None),
            runtime,
        }
    }

    pub fn add_event(&self, message: String) {
        self.events.borrow_mut().push(Event { message });
    }

    pub fn iter_all_events(&self) -> impl Iterator<Item = Event> {
        let mut all_events = Vec::new();
        let mut current_context = Some(self);

        let mut context_chain = Vec::new();
        while let Some(ctx) = current_context {
            context_chain.push(ctx);
            current_context = ctx.parent.as_deref();
        }

        for ctx in context_chain.into_iter().rev() {
            all_events.extend(ctx.events.borrow().clone());
        }

        all_events.into_iter()
    }

    pub fn events_count(&self) -> usize {
        self.events.borrow().len()
    }

    pub fn has_events(&self) -> bool {
        let mut current_context = Some(self);
        while let Some(ctx) = current_context {
            if !ctx.events.borrow().is_empty() {
                return true;
            }
            current_context = ctx.parent.as_deref();
        }
        false
    }

    pub fn has_local_events(&self) -> bool {
        !self.events.borrow().is_empty()
    }

    pub fn get_event(&self, index: usize) -> Option<Event> {
        self.events.borrow().get(index).cloned()
    }

    pub fn last_event(&self) -> Option<Event> {
        self.events.borrow().last().cloned()
    }

    pub fn get_variable(&self, name: &str) -> Option<ExpressionValue> {
        if let Some(value) = self.variables.get(name) {
            Some(value.clone())
        } else if self.is_scope_boundary {
            None
        } else {
            self.parent.as_ref().and_then(|p| p.get_variable(name))
        }
    }

    pub fn declare_variable(&self, name: String, value: ExpressionValue) {
        self.variables.insert(name, value);
    }

    pub fn assign_variable(&self, name: String, value: ExpressionValue) -> Result<(), String> {
        if self.variables.contains_key(&name) {
            self.variables.insert(name, value);
            Ok(())
        } else if self.is_scope_boundary {
            Err(format!("Variable '{}' not found", name))
        } else if let Some(parent) = &self.parent {
            parent.assign_variable(name, value)
        } else {
            Err(format!("Variable '{}' not found", name))
        }
    }

    pub fn create_child(
        parent: Arc<Context>,
        is_scope_boundary: bool,
        runtime: Rc<Runtime>,
    ) -> Self {
        Self {
            parent: Some(parent),
            events: RefCell::new(Vec::new()),
            variables: DashMap::new(),
            is_scope_boundary,
            return_value: RefCell::new(None),
            runtime,
        }
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn runtime_rc(&self) -> Rc<Runtime> {
        self.runtime.clone()
    }

    pub fn set_return_value(&self, value: ExpressionValue) {
        if self.is_scope_boundary {
            *self.return_value.borrow_mut() = Some(value);
        } else if let Some(parent) = &self.parent {
            parent.set_return_value(value);
        }
    }

    pub fn get_return_value(&self) -> Option<ExpressionValue> {
        self.return_value.borrow().clone()
    }

    pub fn has_return_value(&self) -> bool {
        self.return_value.borrow().is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionResult {
    pub params: Option<Vec<ExpressionParameter>>,
    pub value: ExpressionValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionParameter {
    pub name: String,
    pub value: ExpressionValue,
}

impl ExpressionParameter {
    pub fn new(name: String, value: ExpressionValue) -> Self {
        Self { name, value }
    }
}

// was ExprResult is now ExpressionValue
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionValue {
    Unit,
    String(String),
    Boolean(bool),
    List(Arc<ListArray>),
    Option(Option<Box<ExpressionValue>>),
}

impl ExpressionResult {
    pub fn new(value: ExpressionValue) -> Self {
        Self {
            params: None,
            value,
        }
    }

    pub fn with_params(value: ExpressionValue, params: Vec<ExpressionParameter>) -> Self {
        Self {
            params: Some(params),
            value,
        }
    }
}

impl ExpressionValue {
    pub fn as_string(&self) -> Result<&str, String> {
        match self {
            ExpressionValue::String(s) => Ok(s),
            _ => Err("Expected string result".to_string()),
        }
    }

    pub fn as_boolean(&self) -> Result<bool, String> {
        match self {
            ExpressionValue::Boolean(b) => Ok(*b),
            _ => Err("Expected boolean result".to_string()),
        }
    }

    pub fn as_list(&self) -> Result<&Arc<ListArray>, String> {
        match self {
            ExpressionValue::List(list) => Ok(list),
            _ => Err("Expected list result".to_string()),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            ExpressionValue::Unit => "Unit",
            ExpressionValue::String(_) => "String",
            ExpressionValue::Boolean(_) => "Boolean",
            ExpressionValue::List(_) => "List",
            ExpressionValue::Option(_) => "Option",
        }
    }

    pub fn value_string(&self) -> String {
        match self {
            ExpressionValue::Unit => "()".to_string(),
            ExpressionValue::String(s) => s.clone(),
            ExpressionValue::Boolean(b) => b.to_string(),
            ExpressionValue::List(list) => format!("{:?}", list),
            ExpressionValue::Option(opt) => match opt {
                Some(value) => format!("Some({})", value.value_string()),
                None => "None".to_string(),
            },
        }
    }
}

impl std::fmt::Display for ExpressionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value_string())
    }
}
