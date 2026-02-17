use crate::runtime::Runtime;
use crate::runtime::types::{ExpressionParameter, ExpressionResult, ExpressionValue};
use dashmap::DashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Event {
    pub content: ExpressionValue,
    pub name: Option<String>,
    pub params: Option<Vec<ExpressionParameter>>,
}

pub struct Context {
    pub parent: Option<Arc<Context>>,
    events: RefCell<Vec<Event>>,
    pub variables: DashMap<String, ExpressionResult>,
    pub is_scope_boundary: bool,
    return_value: RefCell<Option<ExpressionResult>>,
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

    pub fn add_event(
        &self,
        content: ExpressionValue,
        name: Option<String>,
        params: Option<Vec<ExpressionParameter>>,
    ) {
        self.events.borrow_mut().push(Event {
            content,
            name,
            params,
        });
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

    pub fn get_variable(&self, name: &str) -> Option<ExpressionResult> {
        if let Some(result) = self.variables.get(name) {
            Some(result.clone())
        } else if self.is_scope_boundary {
            None
        } else {
            self.parent.as_ref().and_then(|p| p.get_variable(name))
        }
    }

    pub fn declare_variable(&self, name: String, result: ExpressionResult) {
        self.variables.insert(name, result);
    }

    pub fn assign_variable(&self, name: String, result: ExpressionResult) -> Result<(), String> {
        if self.variables.contains_key(&name) {
            self.variables.insert(name, result);
            Ok(())
        } else if self.is_scope_boundary {
            Err(format!("Variable '{}' not found", name))
        } else if let Some(parent) = &self.parent {
            parent.assign_variable(name, result)
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

    pub fn set_return_value(&self, result: ExpressionResult) {
        if self.is_scope_boundary {
            *self.return_value.borrow_mut() = Some(result);
        } else if let Some(parent) = &self.parent {
            parent.set_return_value(result);
        }
    }

    pub fn get_return_value(&self) -> Option<ExpressionResult> {
        if self.is_scope_boundary {
            self.return_value.borrow().clone()
        } else if let Some(parent) = &self.parent {
            parent.get_return_value()
        } else {
            None
        }
    }

    pub fn has_return_value(&self) -> bool {
        if self.is_scope_boundary {
            self.return_value.borrow_mut().is_some()
        } else if let Some(parent) = &self.parent {
            parent.has_return_value()
        } else {
            false
        }
    }
}
