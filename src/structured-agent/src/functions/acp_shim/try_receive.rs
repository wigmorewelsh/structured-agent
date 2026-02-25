use crate::runtime::ExpressionValue;
use crate::types::{NativeFunction, Parameter, Type};
use async_trait::async_trait;
use std::io;
use std::time::Duration;

#[derive(Debug)]
pub struct TryReceiveFunction {
    parameters: Vec<Parameter>,
    return_type: Type,
}

impl Default for TryReceiveFunction {
    fn default() -> Self {
        Self::new()
    }
}

impl TryReceiveFunction {
    pub fn new() -> Self {
        Self {
            parameters: vec![],
            return_type: Type::string(),
        }
    }
}

#[async_trait(?Send)]
impl NativeFunction for TryReceiveFunction {
    fn name(&self) -> &str {
        "try_receive"
    }

    fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }

    fn return_type(&self) -> &Type {
        &self.return_type
    }

    async fn execute(&self, args: Vec<ExpressionValue>) -> Result<ExpressionValue, String> {
        if !args.is_empty() {
            return Err(format!(
                "try_receive expects 0 arguments, got {}",
                args.len()
            ));
        }

        use std::io::BufRead;
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            let mut buffer = String::new();
            if handle.read_line(&mut buffer).is_ok() {
                let _ = tx.send(buffer.trim().to_string());
            }
        });

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(input) => Ok(ExpressionValue::String(input)),
            Err(_) => Ok(ExpressionValue::String("No prompt received".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_try_receive_function_properties() {
        let try_receive_fn = TryReceiveFunction::new();

        assert_eq!(try_receive_fn.name(), "try_receive");
        assert_eq!(try_receive_fn.parameters().len(), 0);
        assert_eq!(try_receive_fn.return_type().name(), "String");
    }

    #[tokio::test]
    async fn test_try_receive_function_wrong_args_count() {
        let try_receive_fn = TryReceiveFunction::new();

        let result = try_receive_fn
            .execute(vec![ExpressionValue::String("unexpected".to_string())])
            .await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("try_receive expects 0 arguments, got 1")
        );
    }

    #[tokio::test]
    async fn test_try_receive_function_debug() {
        let try_receive_fn = TryReceiveFunction::new();
        let debug_output = format!("{:?}", try_receive_fn);
        assert!(debug_output.contains("TryReceiveFunction"));
    }
}
