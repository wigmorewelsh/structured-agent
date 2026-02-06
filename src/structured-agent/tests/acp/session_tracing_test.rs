use super::test_helpers::{TestAgent, run_local};

#[tokio::test]
async fn test_session_starts_and_runs() {
    let program = r#"
        extern fn print(value: String): ()

        fn main(): () {
            print("Hello from session")
        }
    "#;

    run_local(|| async {
        let agent = TestAgent::with_tracing(program).await;
        let (_result, updates) = agent.wait_with_updates().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(!updates.is_empty(), "Should have captured tracing updates");
        let all_updates = updates.join("\n");
        assert!(
            all_updates.contains("result") || all_updates.contains("function=\"print\""),
            "Should contain result or function print: {}",
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
        let agent1 = TestAgent::with_tracing(program1).await;

        let agent2 = TestAgent::with_tracing(program2).await;

        let result1 = agent1.wait_with_updates();
        let result2 = agent2.wait_with_updates();

        let ((_res1, updates1), (_res2, updates2)) = tokio::join!(result1, result2);

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(!updates1.is_empty(), "Session 1 should have updates");
        assert!(!updates2.is_empty(), "Session 2 should have updates");
    })
    .await;
}
