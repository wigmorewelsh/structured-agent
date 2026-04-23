use std::sync::{Arc, Mutex};

pub use structured_agent_ast::types::{FileId, Span, Spanned};
pub use structured_agent_interpreter_runtime::{
    ExecutableFunction, Function, FunctionProvider, LanguageEngine, PrintEngine, format_event,
};
pub use structured_agent_runtime::{ExternalFunctionDefinition, NativeFunction, Parameter, Type};

#[derive(Debug, Clone)]
pub struct SourceFiles {
    inner: Arc<Mutex<codespan_reporting::files::SimpleFiles<String, String>>>,
}

impl Default for SourceFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceFiles {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(codespan_reporting::files::SimpleFiles::new())),
        }
    }

    pub fn add(&self, name: String, source: String) -> FileId {
        self.inner.lock().unwrap().add(name, source)
    }

    pub fn files(&self) -> Arc<Mutex<codespan_reporting::files::SimpleFiles<String, String>>> {
        self.inner.clone()
    }
}
