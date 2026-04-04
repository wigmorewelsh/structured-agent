use std::sync::Arc;
use structured_agent::cli::config::ProgramSource;
use structured_agent::runtime::Runtime;
use structured_agent_stdlib::unstable::UnstableModule;

#[tokio::test]
async fn test_head_via_use_without_extern_fn() {
    let source = r#"
use unstable::head
use unstable::some_value

fn main(): String {
    let list = ["first", "second", "third"]
    let h = head(list)
    return some_value(h)
}
"#;
    let runtime = Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(UnstableModule))
        .build();
    let result = runtime.run().await.unwrap();
    assert_eq!(result.as_string().unwrap(), "first");
}

#[tokio::test]
async fn test_tail_via_use_without_extern_fn() {
    let source = r#"
use unstable::tail
use unstable::head
use unstable::some_value

fn main(): String {
    let list = ["first", "second", "third"]
    let t = tail(list)
    let inner = some_value(t)
    let h = head(inner)
    return some_value(h)
}
"#;
    let runtime = Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(UnstableModule))
        .build();
    let result = runtime.run().await.unwrap();
    assert_eq!(result.as_string().unwrap(), "second");
}

#[tokio::test]
async fn test_is_some_via_use_without_extern_fn() {
    let source = r#"
use unstable::head
use unstable::is_some

fn main(): Boolean {
    let list = ["x"]
    let h = head(list)
    return is_some(h)
}
"#;
    let runtime = Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(UnstableModule))
        .build();
    let result = runtime.run().await.unwrap();
    assert!(result.as_boolean().unwrap());
}

#[tokio::test]
async fn test_some_value_via_use_without_extern_fn() {
    let source = r#"
use unstable::head
use unstable::some_value

fn main(): String {
    let list = ["extracted"]
    let h = head(list)
    return some_value(h)
}
"#;
    let runtime = Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(UnstableModule))
        .build();
    let result = runtime.run().await.unwrap();
    assert_eq!(result.as_string().unwrap(), "extracted");
}
