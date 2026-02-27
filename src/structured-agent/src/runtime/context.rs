use crate::runtime::Runtime;
use crate::runtime::types::{ExpressionParameter, ExpressionResult, ExpressionValue};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Event {
    pub content: ExpressionValue,
    pub name: Option<String>,
    pub params: Option<Vec<ExpressionParameter>>,
}

pub struct Context {
    parent: Option<Box<Context>>,
    events: Vec<Event>,
    variables: HashMap<String, ExpressionResult>,
    is_scope_boundary: bool,
    return_value: Option<ExpressionResult>,
    runtime: Arc<Runtime>,
}

impl Context {
    pub fn with_runtime(runtime: Arc<Runtime>) -> Self {
        Self {
            parent: None,
            events: Vec::new(),
            variables: HashMap::new(),
            is_scope_boundary: true,
            return_value: None,
            runtime,
        }
    }

    pub fn add_event(
        &mut self,
        content: ExpressionValue,
        name: Option<String>,
        params: Option<Vec<ExpressionParameter>>,
    ) {
        self.events.push(Event {
            content,
            name,
            params,
        });
    }

    pub fn iter_all_events(&self) -> impl Iterator<Item = Event> + '_ {
        let mut all_events = Vec::new();
        let mut current_context = Some(self);

        let mut context_chain = Vec::new();
        while let Some(ctx) = current_context {
            context_chain.push(ctx);
            current_context = ctx.parent.as_deref();
        }

        for ctx in context_chain.into_iter().rev() {
            all_events.extend(ctx.events.clone());
        }

        all_events.into_iter()
    }

    pub fn events_count(&self) -> usize {
        self.events.len()
    }

    pub fn has_events(&self) -> bool {
        let mut current_context = Some(self);
        while let Some(ctx) = current_context {
            if !ctx.events.is_empty() {
                return true;
            }
            current_context = ctx.parent.as_deref();
        }
        false
    }

    pub fn has_local_events(&self) -> bool {
        !self.events.is_empty()
    }

    pub fn get_event(&self, index: usize) -> Option<Event> {
        self.events.get(index).cloned()
    }

    pub fn last_event(&self) -> Option<Event> {
        self.events.last().cloned()
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

    pub fn declare_variable(&mut self, name: String, result: ExpressionResult) {
        self.variables.insert(name, result);
    }

    pub fn assign_variable(
        &mut self,
        name: String,
        result: ExpressionResult,
    ) -> Result<(), String> {
        if self.variables.contains_key(&name) {
            self.variables.insert(name, result);
            Ok(())
        } else if self.is_scope_boundary {
            Err(format!("Variable '{}' not found", name))
        } else if let Some(parent) = &mut self.parent {
            parent.assign_variable(name, result)
        } else {
            Err(format!("Variable '{}' not found", name))
        }
    }

    pub fn remove_variable(&mut self, name: &str) {
        self.variables.remove(name);
    }

    pub fn create_child(self, is_scope_boundary: bool) -> Self {
        let runtime = self.runtime.clone();
        Self {
            parent: Some(Box::new(self)),
            events: Vec::new(),
            variables: HashMap::new(),
            is_scope_boundary,
            return_value: None,
            runtime,
        }
    }

    pub fn restore_parent(self) -> Result<Self, String> {
        self.parent
            .map(|p| *p)
            .ok_or_else(|| "No parent context to restore".to_string())
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn runtime_arc(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    pub fn set_return_value(&mut self, result: ExpressionResult) {
        if self.is_scope_boundary {
            self.return_value = Some(result);
        } else if let Some(parent) = &mut self.parent {
            parent.set_return_value(result);
        }
    }

    pub fn get_return_value(&self) -> Option<ExpressionResult> {
        if self.is_scope_boundary {
            self.return_value.clone()
        } else if let Some(parent) = &self.parent {
            parent.get_return_value()
        } else {
            None
        }
    }

    pub fn has_return_value(&self) -> bool {
        if self.is_scope_boundary {
            self.return_value.is_some()
        } else if let Some(parent) = &self.parent {
            parent.has_return_value()
        } else {
            false
        }
    }
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("events", &self.events)
            .field("variables", &self.variables)
            .field("is_scope_boundary", &self.is_scope_boundary)
            .field("return_value", &self.return_value)
            .field("runtime", &"<Runtime>")
            .finish()
    }
}
