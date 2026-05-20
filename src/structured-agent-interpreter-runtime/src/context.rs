use crate::service::RuntimeService;
use std::sync::Arc;
use structured_agent_runtime::{AgentHandle, ExpressionParameter, ExpressionValue, Source};

#[derive(Debug, Clone)]
pub struct ThinkingEvent {
    pub content: String,
    pub thought_signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActionEvent {
    pub content: ExpressionValue,
    pub name: Option<String>,
    pub params: Option<Vec<ExpressionParameter>>,
    pub source: Source,
}

#[derive(Debug, Clone)]
pub enum ContextEvent {
    Action(ActionEvent),
    Thinking(ThinkingEvent),
}

pub struct Context {
    parent: Option<Box<Context>>,
    events: Vec<ContextEvent>,
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
        source: Source,
    ) {
        self.events.push(ContextEvent::Action(ActionEvent {
            content,
            name,
            params,
            source,
        }));
    }

    pub fn add_thinking_event(&mut self, event: ThinkingEvent) {
        self.events.push(ContextEvent::Thinking(event));
    }

    pub fn iter_all_context_events(&self) -> impl Iterator<Item = ContextEvent> + '_ {
        let mut all = Vec::new();
        let mut chain = Vec::new();
        let mut current = Some(self);
        while let Some(ctx) = current {
            chain.push(ctx);
            current = ctx.parent.as_deref();
        }
        for ctx in chain.into_iter().rev() {
            all.extend(ctx.events.clone());
        }
        all.into_iter()
    }

    pub fn events_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, ContextEvent::Action(_)))
            .count()
    }

    pub fn has_events(&self) -> bool {
        let mut current_context = Some(self);
        while let Some(ctx) = current_context {
            if ctx
                .events
                .iter()
                .any(|e| matches!(e, ContextEvent::Action(_)))
            {
                return true;
            }
            current_context = ctx.parent.as_deref();
        }
        false
    }

    pub fn has_local_events(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, ContextEvent::Action(_)))
    }

    pub fn get_event(&self, index: usize) -> Option<ActionEvent> {
        self.events
            .iter()
            .filter_map(|e| match e {
                ContextEvent::Action(a) => Some(a.clone()),
                ContextEvent::Thinking(_) => None,
            })
            .nth(index)
    }

    pub fn last_event(&self) -> Option<ActionEvent> {
        self.events
            .iter()
            .filter_map(|e| match e {
                ContextEvent::Action(a) => Some(a.clone()),
                ContextEvent::Thinking(_) => None,
            })
            .next_back()
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
