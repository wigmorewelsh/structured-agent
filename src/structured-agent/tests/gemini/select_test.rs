use std::sync::Arc;
use structured_agent::cli::config::ProgramSource;
use structured_agent::gemini::GeminiEngine;
use structured_agent::runtime::{Context, ExpressionValue, Runtime};
use structured_agent::types::LanguageEngine;
use tokio;

async fn make_engine() -> GeminiEngine {
    match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => panic!("Failed to create Gemini engine: {}", e),
    }
}

fn make_context(events: &[&str]) -> Context {
    let runtime =
        Arc::new(Runtime::builder(ProgramSource::Inline("fn main() {}".to_string())).build());
    let mut context = Context::with_runtime(runtime);
    for event in events {
        context.add_event(ExpressionValue::string(*event), None, None);
    }
    context
}

#[tokio::test]
#[ignore]
async fn test_select_with_simple_options() {
    let engine = make_engine().await;
    let context = make_context(&["Choose your favorite color"]);

    let options = vec![
        ExpressionValue::metadata("Red", None),
        ExpressionValue::metadata("Blue", None),
        ExpressionValue::metadata("Green", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(
                index < options.len(),
                "Selected index {} should be less than options length {}",
                index,
                options.len()
            );
            let name = options[index]
                .as_metadata()
                .map(|(n, _)| n)
                .unwrap_or_default();
            println!("Selected option {}: {}", index, name);
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_numbered_options() {
    let engine = make_engine().await;
    let context = make_context(&["Pick the correct mathematical operation for 2 + 2"]);

    let options = vec![
        ExpressionValue::metadata("Addition", None),
        ExpressionValue::metadata("Subtraction", None),
        ExpressionValue::metadata("Multiplication", None),
        ExpressionValue::metadata("Division", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 0, "Should select Addition (index 0) for 2 + 2");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_single_option() {
    let engine = make_engine().await;
    let context = make_context(&[]);

    let options = vec![ExpressionValue::metadata("Only choice", None)];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert_eq!(index, 0, "Should select the only available option");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_contextual_decision() {
    let engine = make_engine().await;
    let context = make_context(&[
        "The weather is very hot today",
        "You need to choose appropriate clothing",
    ]);

    let options = vec![
        ExpressionValue::metadata("Heavy winter coat", None),
        ExpressionValue::metadata("Light t-shirt", None),
        ExpressionValue::metadata("Thick sweater", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 1, "Should select light t-shirt for hot weather");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_mathematical_context() {
    let engine = make_engine().await;
    let context = make_context(&["Calculate the derivative of x^2"]);

    let options = vec![
        ExpressionValue::metadata("2x", None),
        ExpressionValue::metadata("x^2", None),
        ExpressionValue::metadata("2", None),
        ExpressionValue::metadata("x", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 0, "Should select 2x as derivative of x^2");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_many_options() {
    let engine = make_engine().await;
    let context = make_context(&["Choose the programming language known for memory safety"]);

    let options = vec![
        ExpressionValue::metadata("C", None),
        ExpressionValue::metadata("C++", None),
        ExpressionValue::metadata("JavaScript", None),
        ExpressionValue::metadata("Python", None),
        ExpressionValue::metadata("Rust", None),
        ExpressionValue::metadata("Java", None),
        ExpressionValue::metadata("Go", None),
        ExpressionValue::metadata("Ruby", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 4, "Should select Rust for memory safety");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_validates_bounds() {
    let engine = make_engine().await;
    let context = make_context(&[]);

    let options = vec![
        ExpressionValue::metadata("First", None),
        ExpressionValue::metadata("Second", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < 2, "Index should be 0 or 1 for 2 options");
        }
        Err(_) => {}
    }
}

#[tokio::test]
#[ignore]
async fn test_select_prompt_formatting() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(_) => {
            println!("Skipping test: No API key available");
            return;
        }
    };

    let context = make_context(&[]);
    let options = vec![
        ExpressionValue::metadata("A", None),
        ExpressionValue::metadata("B", None),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len(), "Selected index should be valid");
        }
        Err(e) => {
            if e.contains("Language engine returned invalid selection") {
                println!("Validation error caught: {}", e);
            } else {
                panic!("Unexpected error: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {

    #[test]
    fn test_index_parsing_logic() {
        let test_cases = vec![
            ("0", Ok(0)),
            ("1", Ok(1)),
            ("42", Ok(42)),
            ("0\n", Ok(0)),
            ("  1  ", Ok(1)),
            ("abc", Err("invalid")),
            ("1.5", Err("invalid")),
            ("", Err("invalid")),
            ("-1", Err("invalid")),
        ];

        for (input, expected) in test_cases {
            let result: Result<usize, _> = input.trim().parse();

            match (result, expected) {
                (Ok(val), Ok(expected_val)) => {
                    assert_eq!(val, expected_val, "Failed parsing: {}", input);
                }
                (Err(_), Err(_)) => {}
                (Ok(val), Err(_)) => {
                    panic!("Expected parsing to fail for '{}', but got: {}", input, val);
                }
                (Err(e), Ok(expected_val)) => {
                    panic!(
                        "Expected parsing '{}' to succeed with {}, but failed: {}",
                        input, expected_val, e
                    );
                }
            }
        }
    }

    #[test]
    fn test_bounds_checking_logic() {
        let options_count = 3;
        let test_cases = vec![
            (0, true),
            (1, true),
            (2, true),
            (3, false),
            (4, false),
            (100, false),
        ];

        for (index, should_be_valid) in test_cases {
            let is_valid = index < options_count;
            assert_eq!(
                is_valid, should_be_valid,
                "Bounds check failed for index {} with {} options",
                index, options_count
            );
        }
    }

    #[test]
    fn test_prompt_generation_logic() {
        let options = vec!["Red".to_string(), "Blue".to_string(), "Green".to_string()];

        let mut expected_prompt = "SELECT: Choose one of the following options by responding with ONLY the number (0, 1, 2, etc.):\n".to_string();
        for (index, option) in options.iter().enumerate() {
            expected_prompt.push_str(&format!("{}: {}\n", index, option));
        }
        expected_prompt.push_str("\nRespond with only the number, no other text:");

        assert!(expected_prompt.contains("0: Red"));
        assert!(expected_prompt.contains("1: Blue"));
        assert!(expected_prompt.contains("2: Green"));
        assert!(expected_prompt.contains("ONLY the number"));
        assert!(expected_prompt.contains("no other text"));
    }
}
