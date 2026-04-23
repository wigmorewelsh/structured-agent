use crate::service::RuntimeService;
use std::sync::Arc;
use structured_agent_runtime::{AgentHandle, ExpressionParameter, ExpressionValue};

#[derive(Debug, Clone)]
pub struct Event {
    pub content: ExpressionValue,
    pub name: Option<String>,
    pub params: Option<Vec<ExpressionParameter>>,
}

pub struct Context {
    parent: Option<Box<Context>>,
    events: Vec<Event>,
    runtime: Arc<dyn RuntimeService>,
    agent_handle: AgentHandle,
}

impl Context {
    pub fn with_runtime(runtime: Arc<dyn RuntimeService>) -> Self {
        Self {
            parent: None,
            events: Vec::new(),
            runtime,
            agent_handle: AgentHandle::detached(),
        }
    }

    pub fn with_runtime_and_handle(
        runtime: Arc<dyn RuntimeService>,
        agent_handle: AgentHandle,
    ) -> Self {
        Self {
            parent: None,
            events: Vec::new(),
            runtime,
            agent_handle,
        }
    }

    pub fn agent_handle(&self) -> &AgentHandle {
        &self.agent_handle
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

    pub fn create_child(self) -> Self {
        let runtime = self.runtime.clone();
        let agent_handle = self.agent_handle.clone();
        Self {
            parent: Some(Box::new(self)),
            events: Vec::new(),
            runtime,
            agent_handle,
        }
    }

    pub fn restore_parent(self) -> Result<Self, String> {
        self.parent
            .map(|p| *p)
            .ok_or_else(|| "No parent context to restore".to_string())
    }

    pub fn runtime(&self) -> &dyn RuntimeService {
        self.runtime.as_ref()
    }

    pub fn runtime_arc(&self) -> Arc<dyn RuntimeService> {
        self.runtime.clone()
    }
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("events", &self.events)
            .field("runtime", &"<Runtime>")
            .field("agent_handle", &"<AgentHandle>")
            .finish()
    }
}
