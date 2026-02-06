use super::test_helpers::{TestAgent, run_local};

#[tokio::test]
async fn test_receive_function_gets_prompt() {
    let program = r#"
        extern fn print(value: String): ()
        extern fn receive(): String

        fn main(): () {
            let message = receive()
            print(message)
        }
    "#;

    run_local(|| async {
        let agent = TestAgent::from_program(program).await;

        tokio::task::spawn_local(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            agent.send_prompt("test message").await;
            agent.wait().await;
        });
    })
    .await;
}

#[tokio::test]
async fn test_receive_multiple_prompts() {
    let program = r#"
        extern fn print(value: String): ()
        extern fn receive(): String

        fn main(): () {
            let first = receive()
            print(first)
            let second = receive()
            print(second)
        }
    "#;

    run_local(|| async {
        let agent = TestAgent::from_program(program).await;

        tokio::task::spawn_local(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            agent.send_prompt("first message").await;
            agent.send_prompt("second message").await;
            agent.wait().await;
        });
    })
    .await;
}
