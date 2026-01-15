mod ast;
mod compiler;
mod expressions;
mod parser;
mod types;

use combine::EasyParser;
use compiler::Compiler;
use expressions::Expression;
use parser::*;
use types::Context;

fn main() {
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
                        let mut context = Context::new();

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
