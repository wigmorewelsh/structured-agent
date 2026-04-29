pub use structured_agent_il::BytecodeRef;

pub mod context;
pub mod service;
pub mod traits;

pub use context::{ActionEvent, Context, ContextEvent, ThinkingEvent};
pub use service::RuntimeService;
pub use traits::{
    Event, ExecutableFunction, FillParameterEvent, Function, FunctionProvider, LanguageEngine,
    PrintEngine, SelectEvent, TypedEvent,
};

pub use structured_agent_runtime::{
    ActorMailboxReceiver, ActorMessage, ActorRef, ActorRegistry, AgentError, AgentHandle, AgentId,
    AgentMessage, AgentMessageContent, DefinitionPath, ExpressionParameter, ExpressionResult,
    ExpressionValue, ExternalFunctionDefinition, Parameter, RuntimeError, Type,
};
