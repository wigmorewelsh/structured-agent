mod acp_test_helpers;

use acp_test_helpers::{TracingTestAgent, run_local};
use agent_client_protocol as acp;
use std::sync::Arc;
use structured_agent::runtime::Runtime;

#[tokio::test]
async fn test_session_starts_and_runs() {
    let program = r#"
        extern fn print(value: String): ()

        fn main(): () {
            print("Hello from session")
        }
    "#;

    run_local(|| async {
        let agent = TracingTestAgent::from_program(program).await;
        let (_result, updates) = agent.wait().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(!updates.is_empty(), "Should have captured tracing updates");
        let all_updates = updates.join("\n");
        assert!(
            all_updates.contains("function_call") || all_updates.contains("session"),
            "Should contain function call or session span: {}",
            all_updates
        );
    })
    .await;
}

#[tokio::test]
async fn test_multiple_sessions_independent_tracing() {
    let program1 = r#"
        extern fn print(value: String): ()

        fn main(): () {
            print("Session 1 message")
        }
    "#;

    let program2 = r#"
        extern fn print(value: String): ()

        fn main(): () {
            print("Session 2 message")
        }
    "#;

    run_local(|| async {
        let runtime1 = Runtime::builder()
            .with_native_function(Arc::new(structured_agent::functions::PrintFunction::new()))
            .build();

        let runtime2 = Runtime::builder()
            .with_native_function(Arc::new(structured_agent::functions::PrintFunction::new()))
            .build();

        let agent1 = TracingTestAgent::from_runtime(
            runtime1,
            program1,
            acp::SessionId::new("session-1".to_string()),
        )
        .await;

        let agent2 = TracingTestAgent::from_runtime(
            runtime2,
            program2,
            acp::SessionId::new("session-2".to_string()),
        )
        .await;

        let result1 = agent1.wait();
        let result2 = agent2.wait();

        let ((res1, updates1), (res2, updates2)) = tokio::join!(result1, result2);

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(!updates1.is_empty(), "Session 1 should have updates");
        assert!(!updates2.is_empty(), "Session 2 should have updates");
    })
    .await;
}
