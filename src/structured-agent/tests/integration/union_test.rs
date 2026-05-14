use std::any::Any;

use structured_agent::cli::config::ProgramSource;
use structured_agent::runtime::Runtime;
use structured_agent_interpreter_runtime::{
    Context, ExecutableFunction, ExpressionResult, ExpressionValue, Function, Parameter, Type,
};

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

#[async_trait::async_trait]
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
async fn match_on_image_takes_image_arm() {
    let source = r#"
type Media = Image | Audio
extern fn make_image(): Media

fn main(): String {
    return match make_image() { Image(img) => "got image", Audio(a) => "got audio" }
}
"#;
    let mut runtime = Runtime::builder(ProgramSource::Inline(source.to_string())).build();
    runtime.register_function(Box::new(NativeValueFn::new(
        "make_image",
        Type::image(),
        ExpressionValue::image("image/png", vec![]),
    )));
    let result = runtime.run().await.expect("Program execution failed");
    assert_eq!(result.as_string().unwrap(), "got image");
}

#[tokio::test]
async fn match_on_audio_takes_audio_arm() {
    let source = r#"
type Media = Image | Audio
extern fn make_audio(): Media

fn main(): String {
    return match make_audio() { Image(img) => "got image", Audio(a) => "got audio" }
}
"#;
    let mut runtime = Runtime::builder(ProgramSource::Inline(source.to_string())).build();
    runtime.register_function(Box::new(NativeValueFn::new(
        "make_audio",
        Type::audio(),
        ExpressionValue::audio("audio/mp3", vec![]),
    )));
    let result = runtime.run().await.expect("Program execution failed");
    assert_eq!(result.as_string().unwrap(), "got audio");
}

#[tokio::test]
async fn match_via_alias_executes_correctly() {
    let source = r#"
type Media = Image | Audio
extern fn make_media(): Media

fn main(): String {
    return match make_media() { Image(img) => "got image", Audio(a) => "got audio" }
}
"#;
    let mut runtime = Runtime::builder(ProgramSource::Inline(source.to_string())).build();
    runtime.register_function(Box::new(NativeValueFn::new(
        "make_media",
        Type::image(),
        ExpressionValue::image("image/png", vec![]),
    )));
    let result = runtime.run().await.expect("Program execution failed");
    assert_eq!(result.as_string().unwrap(), "got image");
}
