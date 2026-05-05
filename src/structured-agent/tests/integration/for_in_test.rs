use super::helpers::run_program_with_iterator;

#[tokio::test]
async fn for_in_iterates_all_elements() {
    let result = run_program_with_iterator(
        r#"
        fn main(): String {
            let items = ["a", "b", "c"]
            let last = ""
            for x in items.iter() {
                last = x
            }
            return last
        }
    "#,
    )
    .await;
    assert_eq!(result.as_string().unwrap(), "c");
}

#[tokio::test]
async fn for_in_single_element() {
    let result = run_program_with_iterator(
        r#"
        fn main(): String {
            let items = ["only"]
            let result = "none"
            for x in items.iter() {
                result = x
            }
            return result
        }
    "#,
    )
    .await;
    assert_eq!(result.as_string().unwrap(), "only");
}

#[tokio::test]
async fn for_in_body_not_entered_when_one_element_matches() {
    let result = run_program_with_iterator(
        r#"
        fn main(): String {
            let items = ["first", "second"]
            let result = "none"
            for x in items.iter() {
                result = x
            }
            return result
        }
    "#,
    )
    .await;
    assert_eq!(result.as_string().unwrap(), "second");
}
