use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::any::Any;
use std::sync::{Arc, Mutex};

use structured_agent::cli::config::ProgramSource;
use structured_agent::gemini::GeminiEngine;
use structured_agent::runtime::Runtime;
use structured_agent_interpreter_runtime::{
    ActionEvent, Context, ContextEvent, Event, ExecutableFunction, ExpressionResult,
    ExpressionValue, Function, LanguageEngine, Parameter, ThinkingEvent, Type,
};

struct RecordingEngine {
    events: Mutex<Vec<ActionEvent>>,
    return_value: ExpressionValue,
}

impl RecordingEngine {
    fn new(return_value: ExpressionValue) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            return_value,
        }
    }

    fn last_action_event(&self) -> Option<ActionEvent> {
        self.events.lock().unwrap().last().cloned()
    }
}

#[async_trait]
impl LanguageEngine for RecordingEngine {
    async fn request(
        &self,
        context: &Context,
        _request: &dyn Event,
    ) -> Result<(ExpressionValue, Option<ThinkingEvent>), String> {
        let action_events: Vec<ActionEvent> = context
            .iter_all_context_events()
            .filter_map(|e| match e {
                ContextEvent::Action(a) => Some(a),
                _ => None,
            })
            .collect();
        *self.events.lock().unwrap() = action_events;
        Ok((self.return_value.clone(), None))
    }
}

#[derive(Debug, Clone)]
struct NativeValueFn {
    fn_name: String,
    return_type: Type,
    return_value: ExpressionValue,
}

impl NativeValueFn {
    fn new(name: &str, return_type: Type, return_value: ExpressionValue) -> Self {
        Self {
            fn_name: name.to_string(),
            return_type,
            return_value,
        }
    }
}

#[async_trait]
impl Function for NativeValueFn {
    fn name(&self) -> &str {
        &self.fn_name
    }

    fn parameters(&self) -> &[Parameter] {
        &[]
    }

    fn function_return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(
        &self,
        context: Context,
        _args: Vec<ExpressionResult>,
    ) -> Result<(Context, ExpressionResult), String> {
        Ok((context, ExpressionResult::new(self.return_value.clone())))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Function> {
        Box::new(self.clone())
    }
}

impl ExecutableFunction for NativeValueFn {
    fn clone_executable(&self) -> Box<dyn ExecutableFunction> {
        Box::new(self.clone())
    }
}

#[tokio::test]
async fn image_value_flows_through_engine_as_inline_data_part() {
    let source = r#"
extern fn capture_image(): Image

fn describe(): String {
    capture_image()!
}

fn main(): String {
    return describe()
}
"#;
    let recording_engine = Arc::new(RecordingEngine::new(ExpressionValue::string(
        "it is an image",
    )));
    let engine_clone = Arc::clone(&recording_engine) as Arc<dyn LanguageEngine>;

    let mut runtime = Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_language_engine(engine_clone)
        .build();

    runtime.register_function(Box::new(NativeValueFn::new(
        "capture_image",
        Type::image(),
        ExpressionValue::image("image/png", vec![1, 2, 3]),
    )));

    let result = runtime.run().await.expect("Program execution failed");
    assert_eq!(result.as_string().unwrap(), "it is an image");

    let event = recording_engine
        .last_action_event()
        .expect("No events recorded");
    event.content.as_image().expect("Expected image content");

    let part = GeminiEngine::expression_value_to_part(&event.content)
        .expect("Expected part from image value");
    let inline_data = part.inline_data.as_ref().expect("Expected inline_data");
    assert_eq!(inline_data.mime_type, "image/png");
    assert_eq!(inline_data.data, STANDARD.encode([1u8, 2, 3]));
}

#[tokio::test]
async fn audio_value_flows_through_engine_as_inline_data_part() {
    let source = r#"
extern fn capture_audio(): Audio

fn describe(): String {
    capture_audio()!
}

fn main(): String {
    return describe()
}
"#;
    let recording_engine = Arc::new(RecordingEngine::new(ExpressionValue::string("it is audio")));
    let engine_clone = Arc::clone(&recording_engine) as Arc<dyn LanguageEngine>;

    let mut runtime = Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_language_engine(engine_clone)
        .build();

    runtime.register_function(Box::new(NativeValueFn::new(
        "capture_audio",
        Type::audio(),
        ExpressionValue::audio("audio/mp3", vec![4, 5, 6]),
    )));

    let result = runtime.run().await.expect("Program execution failed");
    assert_eq!(result.as_string().unwrap(), "it is audio");

    let event = recording_engine
        .last_action_event()
        .expect("No events recorded");
    event.content.as_audio().expect("Expected audio content");

    let part = GeminiEngine::expression_value_to_part(&event.content)
        .expect("Expected part from audio value");
    let inline_data = part.inline_data.as_ref().expect("Expected inline_data");
    assert_eq!(inline_data.mime_type, "audio/mp3");
    assert_eq!(inline_data.data, STANDARD.encode([4u8, 5, 6]));
}
