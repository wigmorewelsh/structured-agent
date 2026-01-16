mod ast;
mod compiler;
mod expressions;
mod gemini;

mod runtime;
mod types;

use runtime::Runtime;

#[tokio::main]
async fn main() {
    let input = r#"
fn hello_world() -> () {
    "Hello from the structured agent!"!
    "This demonstrates the new compiler architecture"!
}

fn main() -> () {
    "Starting program execution"!
    let result = hello_world()
    "Program completed successfully"!
}
"#;

    let runtime = Runtime::new();

    match runtime.run(input).await {
        Ok(result) => {
            println!("Program executed successfully");
            println!("Result: {:?}", result);
        }
        Err(e) => {
            println!("Execution failed: {:?}", e);
        }
    }
}
