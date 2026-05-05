use std::sync::Arc;

use structured_agent::cli::config::ProgramSource;
use structured_agent::runtime::Runtime;
use structured_agent_stdlib::actor::ActorModule;
use structured_agent_stdlib::prelude::PreludeModule;

async fn run_actor_program(source: &str) -> structured_agent::runtime::ExpressionValue {
    Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(ActorModule))
        .build()
        .run()
        .await
        .expect("Program execution failed")
}

async fn run_actor_program_with_prelude(
    source: &str,
) -> structured_agent::runtime::ExpressionValue {
    Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(ActorModule))
        .with_module(Arc::new(PreludeModule))
        .build()
        .run()
        .await
        .expect("Program execution failed")
}

#[tokio::test]
async fn actor_spawn_and_call_returns_value() {
    let source = r#"
        mod Counter {
            fn increment(): String {
                return "incremented"
            }
        }

        fn main(): String {
            let c = spawn<Counter>("c1")
            return c.increment()
        }
    "#;

    let value = run_actor_program(source).await;
    assert_eq!(value.as_string().unwrap(), "incremented");
}

#[tokio::test]
async fn actor_call_with_argument() {
    let source = r#"
        mod Greeter {
            fn greet(name: String): String {
                return name
            }
        }

        fn main(): String {
            let g = spawn<Greeter>("g1")
            return g.greet("world")
        }
    "#;

    let value = run_actor_program(source).await;
    assert_eq!(value.as_string().unwrap(), "world");
}

#[tokio::test]
async fn actor_method_with_generic_trait_bound() {
    let source = r#"
        mod Stringifier {
            fn stringify<T: ToString>(val: T): String {
                return val.to_string()
            }
        }

        fn main(): String {
            let s = spawn<Stringifier>("s1")
            return s.stringify(42)
        }
    "#;

    let value = run_actor_program_with_prelude(source).await;
    assert_eq!(value.as_string().unwrap(), "42");
}

#[tokio::test]
async fn actor_method_with_multiple_args_and_trait() {
    let source = r#"
        mod Formatter {
            fn format<T: ToString>(label: String, val: T): String {
                return label
            }
        }

        fn main(): String {
            let f = spawn<Formatter>("f1")
            return f.format("ok", 99)
        }
    "#;

    let value = run_actor_program_with_prelude(source).await;
    assert_eq!(value.as_string().unwrap(), "ok");
}
