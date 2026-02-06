use crate::acp::agent::PromptMessage;
use crate::runtime::ExprResult;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info};

#[derive(Debug)]
pub struct ReceiveFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
    prompt_rx: Arc<Mutex<mpsc::UnboundedReceiver<PromptMessage>>>,
}

impl ReceiveFunction {
    pub fn new(prompt_rx: mpsc::UnboundedReceiver<PromptMessage>) -> Self {
        Self {
            parameters: vec![],
            return_type: Type::string(),
            prompt_rx: Arc::new(Mutex::new(prompt_rx)),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for ReceiveFunction {
    fn name(&self) -> &str {
        "receive"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExprResult>) -> Result<ExprResult, String> {
        if !args.is_empty() {
            error!(
                "receive called with wrong number of arguments: {}",
                args.len()
            );
            return Err(format!("receive expects 0 arguments, got {}", args.len()));
        }

        debug!("receive() called, waiting for prompt");
        let mut rx = self.prompt_rx.lock().await;

        match rx.recv().await {
            Some(message) => {
                debug!("Received prompt: {}", message.content);
                let content = message.content.clone();
                let _ = message.response_tx.send(());
                debug!("Prompt response sent");
                Ok(ExprResult::String(content))
            }
            None => {
                error!("Prompt channel closed");
                Err("Prompt channel closed".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_receive_function_properties() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let receive_fn = ReceiveFunction::new(rx);

        assert_eq!(receive_fn.name(), "receive");
        assert_eq!(receive_fn.parameters().len(), 0);
        assert_eq!(receive_fn.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_receive_function_execute() {
        let (tx, rx) = mpsc::unbounded_channel();
        let receive_fn = ReceiveFunction::new(rx);

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let message = PromptMessage {
            content: "test prompt".to_string(),
            response_tx,
        };

        tx.send(message).unwrap();

        let result = receive_fn.execute(vec![]).await.unwrap();
        assert_eq!(result, ExprResult::String("test prompt".to_string()));

        assert!(response_rx.await.is_ok());
    }

    #[tokio::test]
    async fn test_receive_function_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel();
        let receive_fn = ReceiveFunction::new(rx);

        drop(tx);

        let result = receive_fn.execute(vec![]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Prompt channel closed"));
    }

    #[tokio::test]
    async fn test_receive_function_wrong_args_count() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let receive_fn = ReceiveFunction::new(rx);

        let result = receive_fn
            .execute(vec![ExprResult::String("unexpected".to_string())])
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("receive expects 0 arguments, got 1")
        );
    }

    #[tokio::test]
    async fn test_receive_multiple_prompts() {
        let (tx, rx) = mpsc::unbounded_channel();
        let receive_fn = ReceiveFunction::new(rx);

        let (response_tx1, response_rx1) = tokio::sync::oneshot::channel();
        tx.send(PromptMessage {
            content: "first".to_string(),
            response_tx: response_tx1,
        })
        .unwrap();

        let (response_tx2, response_rx2) = tokio::sync::oneshot::channel();
        tx.send(PromptMessage {
            content: "second".to_string(),
            response_tx: response_tx2,
        })
        .unwrap();

        let result1 = receive_fn.execute(vec![]).await.unwrap();
        assert_eq!(result1, ExprResult::String("first".to_string()));
        assert!(response_rx1.await.is_ok());

        let result2 = receive_fn.execute(vec![]).await.unwrap();
        assert_eq!(result2, ExprResult::String("second".to_string()));
        assert!(response_rx2.await.is_ok());
    }
}
