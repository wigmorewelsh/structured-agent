mod ast;
mod compiler;
mod expressions;
mod gemini;

mod runtime;
mod types;

use combine::EasyParser;
use compiler::Compiler;
use compiler::parser::*;
use expressions::Expression;
use gemini::{GeminiConfig, GeminiEngine};
use runtime::Runtime;
use runtime::{Context, ExprResult};
use std::rc::Rc;

fn main() {
    // Example 1: Using default PrintEngine
    println!("=== Example 1: Default PrintEngine ===");
    run_with_default_engine();

    // Example 2: Using GeminiEngine (if configured)
    println!("\n=== Example 2: GeminiEngine ===");
    run_with_gemini_engine();
}

fn run_with_default_engine() {
    let input = r#"
fn analyze_code(context: Context, code: String) -> Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}

fn suggest_fix(context: Context, analysis: Analysis) -> CodeFix {
    "Given this analysis, suggest a fix"!
    analysis!
}

fn main() -> () {
    let context = Context()
    let code = "fn div(a, b) = a / b"

    let issues = analyze_code(context, code)

    let fixes = suggest_fix(context, issues)

    fixes
}
"#;

    match parse_program().easy_parse(input) {
        Ok((functions, _)) => {
            println!("Parsed {} functions successfully", functions.len());

            if let Some(func) = functions.first() {
                match Compiler::compile_function(func) {
                    Ok(compiled) => {
                        let runtime = Rc::new(Runtime::new());
                        let mut context = Context::with_runtime(runtime);

                        match compiled.evaluate(&mut context) {
                            Ok(_) => println!(
                                "Execution successful - {} events generated",
                                context.events.len()
                            ),
                            Err(e) => println!("Execution failed: {}", e),
                        }
                    }
                    Err(e) => println!("Compilation failed: {}", e),
                }
            }
        }
        Err(e) => println!("Parse error: {:?}", e),
    }
}

fn run_with_gemini_engine() {
    // Try to create a GeminiEngine from environment variables
    match GeminiEngine::from_env() {
        Ok(gemini_engine) => {
            println!("Successfully created GeminiEngine from environment");

            let mut context = Context::with_engine(Rc::new(gemini_engine));

            // Add some context information
            context.add_event("User is analyzing Rust code for potential issues".to_string());
            context.set_variable(
                "language".to_string(),
                ExprResult::String("Rust".to_string()),
            );

            // Use the engine to generate a response
            let response = context.runtime().engine().untyped(&context);
            println!("Gemini response: {}", response);
        }
        Err(e) => {
            println!("Failed to create GeminiEngine: {}", e);
            println!(
                "Make sure GEMINI_API_KEY environment variable is set or gcloud is configured"
            );

            // Try with explicit config as fallback
            if let Ok(api_key) = std::env::var("GEMINI_API_KEY") {
                let config = GeminiConfig::with_api_key(
                    "gemini-api".to_string(),
                    "global".to_string(),
                    api_key,
                );
                match GeminiEngine::new(config) {
                    Ok(gemini_engine) => {
                        println!("Successfully created GeminiEngine with API key");

                        let mut context = Context::with_engine(Rc::new(gemini_engine));
                        context.add_event("Testing Gemini integration".to_string());

                        let response = context.runtime().engine().untyped(&context);
                        println!("Gemini response: {}", response);
                    }
                    Err(e2) => println!("Also failed with explicit API key: {}", e2),
                }
            }
        }
    }
}
