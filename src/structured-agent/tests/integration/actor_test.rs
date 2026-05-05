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
async fn actor_id_reflects_spawn_key() {
    let source = r#"
        mod Counter(a: actor) {
            use a::actor_id

            fn get_id(): String {
                return actor_id()
            }
        }

        fn main(): String {
            let c = spawn<Counter>("my_counter")
            return c.get_id()
        }
    "#;

    let value = run_actor_program(source).await;
    assert_eq!(value.as_string().unwrap(), "main::Counter:my_counter");
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
