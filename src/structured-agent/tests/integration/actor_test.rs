use std::sync::Arc;

use structured_agent::cli::config::ProgramSource;
use structured_agent::runtime::Runtime;
use structured_agent_stdlib::actor::ActorModule;

async fn run_actor_program(source: &str) -> structured_agent::runtime::ExpressionValue {
    Runtime::builder(ProgramSource::Inline(source.to_string()))
        .with_module(Arc::new(ActorModule))
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
