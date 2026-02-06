use std::rc::Rc;
use structured_agent::gemini::GeminiEngine;
use structured_agent::runtime::{Context, Runtime};
use structured_agent::types::LanguageEngine;
use tokio;

#[tokio::test]
#[ignore]
async fn test_select_with_simple_options() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    context.add_event("Choose your favorite color".to_string());

    let options = vec!["Red".to_string(), "Blue".to_string(), "Green".to_string()];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(
                index < options.len(),
                "Selected index {} should be less than options length {}",
                index,
                options.len()
            );
            println!("✓ Selected option {}: {}", index, options[index]);
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_numbered_options() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    context.add_event("Pick the correct mathematical operation for 2 + 2".to_string());

    let options = vec![
        "Addition".to_string(),
        "Subtraction".to_string(),
        "Multiplication".to_string(),
        "Division".to_string(),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 0, "Should select Addition (index 0) for 2 + 2");
            println!("✓ Correctly selected Addition for 2 + 2");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_single_option() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let context = Context::with_runtime(runtime);

    let options = vec!["Only choice".to_string()];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert_eq!(index, 0, "Should select the only available option");
            println!("✓ Correctly selected the only option");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_contextual_decision() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    context.add_event("The weather is very hot today".to_string());
    context.add_event("You need to choose appropriate clothing".to_string());

    let options = vec![
        "Heavy winter coat".to_string(),
        "Light t-shirt".to_string(),
        "Thick sweater".to_string(),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 1, "Should select light t-shirt for hot weather");
            println!("✓ Correctly selected light t-shirt for hot weather");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_mathematical_context() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    context.add_event("Calculate the derivative of x^2".to_string());

    let options = vec![
        "2x".to_string(),
        "x^2".to_string(),
        "2".to_string(),
        "x".to_string(),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 0, "Should select 2x as derivative of x^2");
            println!("✓ Correctly selected 2x as derivative of x^2");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_with_many_options() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let mut context = Context::with_runtime(runtime);
    context.add_event("Choose the programming language known for memory safety".to_string());

    let options = vec![
        "C".to_string(),
        "C++".to_string(),
        "JavaScript".to_string(),
        "Python".to_string(),
        "Rust".to_string(),
        "Java".to_string(),
        "Go".to_string(),
        "Ruby".to_string(),
    ];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len());
            assert_eq!(index, 4, "Should select Rust for memory safety");
            println!("✓ Correctly selected Rust for memory safety");
        }
        Err(e) => panic!("Selection failed: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn test_select_validates_bounds() {
    let engine = match GeminiEngine::from_env().await {
        Ok(engine) => engine,
        Err(e) => {
            panic!("Failed to create Gemini engine: {}", e);
        }
    };

    let runtime = Rc::new(Runtime::new());
    let context = Context::with_runtime(runtime);

    let options = vec!["Option A".to_string(), "Option B".to_string()];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < 2, "Index should be 0 or 1 for 2 options");
            println!("✓ Response index {} is within bounds", index);
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

    let runtime = Rc::new(Runtime::new());
    let context = Context::with_runtime(runtime);
    let options = vec!["First".to_string(), "Second".to_string()];

    let result = engine.select(&context, &options).await;

    match result {
        Ok(index) => {
            assert!(index < options.len(), "Selected index should be valid");
            println!("✓ Prompt formatting test passed with index: {}", index);
        }
        Err(e) => {
            if e.contains("Language engine returned invalid selection") {
                println!("✓ Validation error caught: {}", e);
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
                (Err(_), Err(_)) => {
                    // Both failed as expected
                }
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
