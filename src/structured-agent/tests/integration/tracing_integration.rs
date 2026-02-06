use std::rc::Rc;
use std::sync::{Arc, Mutex};
use structured_agent::expressions::{
    CallExpr, FunctionExpr, InjectionExpr, PlaceholderExpr, SelectClauseExpr, SelectExpr,
    StringLiteralExpr, VariableExpr,
};
use structured_agent::runtime::{Context, Runtime};
use structured_agent::types::{Expression, Parameter, Type};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct TestTracingLayer {
    spans: Arc<Mutex<Vec<String>>>,
    events: Arc<Mutex<Vec<String>>>,
}

impl<S> tracing_subscriber::Layer<S> for TestTracingLayer
where
    S: tracing::Subscriber,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = SpanVisitor {
            message: String::new(),
        };
        attrs.record(&mut visitor);
        self.spans.lock().unwrap().push(visitor.message);
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = EventVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);
        self.events.lock().unwrap().push(visitor.message);
    }
}

struct SpanVisitor {
    message: String,
}

impl tracing::field::Visit for SpanVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.message.is_empty() {
            self.message.push_str(", ");
        }
        self.message
            .push_str(&format!("{}={:?}", field.name(), value));
    }
}

struct EventVisitor {
    message: String,
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.message.is_empty() {
            self.message.push_str(", ");
        }
        self.message
            .push_str(&format!("{}={:?}", field.name(), value));
    }
}

#[tokio::test]
async fn test_function_call_tracing() {
    let spans = Arc::new(Mutex::new(Vec::new()));
    let events = Arc::new(Mutex::new(Vec::new()));

    let test_layer = TestTracingLayer {
        spans: spans.clone(),
        events: events.clone(),
    };

    let _guard = tracing_subscriber::registry()
        .with(test_layer)
        .set_default();

    let mut runtime = Runtime::new();
    let function_info = FunctionExpr {
        name: "greet".to_string(),
        parameters: vec![],
        return_type: Type::string(),
        body: vec![Box::new(StringLiteralExpr {
            value: "Hello".to_string(),
        })],
        documentation: None,
    };
    runtime.register_function(function_info);

    let runtime = Rc::new(runtime);
    let context = Arc::new(Context::with_runtime(runtime));

    let expr = CallExpr {
        function: "greet".to_string(),
        arguments: vec![],
    };

    let _result = expr.evaluate(context).await.unwrap();

    let recorded_spans = spans.lock().unwrap();
    let recorded_events = events.lock().unwrap();

    assert!(recorded_spans.iter().any(|s| s.contains("greet")));
    assert!(
        recorded_events
            .iter()
            .any(|e| e.contains("function_result"))
    );
}

#[tokio::test]
async fn test_injection_tracing() {
    let events = Arc::new(Mutex::new(Vec::new()));

    let test_layer = TestTracingLayer {
        spans: Arc::new(Mutex::new(Vec::new())),
        events: events.clone(),
    };

    let _guard = tracing_subscriber::registry()
        .with(test_layer)
        .set_default();

    let runtime = Rc::new(Runtime::new());
    let context = Arc::new(Context::with_runtime(runtime));

    let expr = InjectionExpr {
        inner: Box::new(StringLiteralExpr {
            value: "test content".to_string(),
        }),
    };

    let _result = expr.evaluate(context).await.unwrap();

    let recorded_events = events.lock().unwrap();
    assert!(recorded_events.iter().any(|e| e.contains("injection")));
}

#[tokio::test]
async fn test_select_tracing() {
    let events = Arc::new(Mutex::new(Vec::new()));

    let test_layer = TestTracingLayer {
        spans: Arc::new(Mutex::new(Vec::new())),
        events: events.clone(),
    };

    let _guard = tracing_subscriber::registry()
        .with(test_layer)
        .set_default();

    let mut runtime = Runtime::new();

    let function_info = FunctionExpr {
        name: "option_one".to_string(),
        parameters: vec![],
        return_type: Type::string(),
        body: vec![Box::new(StringLiteralExpr {
            value: "one".to_string(),
        })],
        documentation: Some("First option".to_string()),
    };
    runtime.register_function(function_info);

    let runtime = Rc::new(runtime);
    let context = Arc::new(Context::with_runtime(runtime));

    let clause = SelectClauseExpr {
        expression_to_run: Box::new(FunctionExpr {
            name: "option_one".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "one".to_string(),
            })],
            documentation: Some("First option".to_string()),
        }),
        result_variable: "result".to_string(),
        expression_next: Box::new(VariableExpr {
            name: "result".to_string(),
        }),
    };

    let select_expr = SelectExpr {
        clauses: vec![clause],
    };

    let _result = select_expr.evaluate(context).await.unwrap();

    let recorded_events = events.lock().unwrap();
    assert!(recorded_events.iter().any(|e| e.contains("select_clause")));
}

#[tokio::test]
async fn test_placeholder_filling_tracing() {
    let events = Arc::new(Mutex::new(Vec::new()));

    let test_layer = TestTracingLayer {
        spans: Arc::new(Mutex::new(Vec::new())),
        events: events.clone(),
    };

    let _guard = tracing_subscriber::registry()
        .with(test_layer)
        .set_default();

    let mut runtime = Runtime::new();

    let function_info = FunctionExpr {
        name: "process".to_string(),
        parameters: vec![Parameter::new("data".to_string(), Type::string())],
        return_type: Type::string(),
        body: vec![Box::new(StringLiteralExpr {
            value: "processed".to_string(),
        })],
        documentation: None,
    };
    runtime.register_function(function_info);

    let runtime = Rc::new(runtime);
    let context = Arc::new(Context::with_runtime(runtime));

    context.add_event("Some context data".to_string());

    let expr = CallExpr {
        function: "process".to_string(),
        arguments: vec![Box::new(PlaceholderExpr {})],
    };

    let _result = expr.evaluate(context).await.unwrap();

    let recorded_events = events.lock().unwrap();
    assert!(
        recorded_events
            .iter()
            .any(|e| e.contains("placeholder_filled"))
    );
}
