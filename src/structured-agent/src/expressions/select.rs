use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use tracing::info;

pub struct SelectClauseExpr {
    pub expression_to_run: Box<dyn Expression>,
    pub result_variable: String,
    pub expression_next: Box<dyn Expression>,
}

impl std::fmt::Debug for SelectClauseExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectClauseExpr")
            .field("result_variable", &self.result_variable)
            .finish()
    }
}

pub struct SelectExpr {
    pub clauses: Vec<SelectClauseExpr>,
}

impl std::fmt::Debug for SelectExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectExpr")
            .field("clauses", &self.clauses.len())
            .finish()
    }
}

#[async_trait(?Send)]
impl Expression for SelectExpr {
    async fn evaluate(&self, context: Arc<Context>) -> Result<ExprResult, String> {
        if self.clauses.is_empty() {
            return Err("Select statement must have at least one clause".to_string());
        }

        let mut clause_descriptions = Vec::new();
        for clause in &self.clauses {
            let description = if let Some(doc) = clause.expression_to_run.documentation() {
                format!(
                    "Execute function '{}' and store result as '{}': {}",
                    clause.result_variable, clause.result_variable, doc
                )
            } else {
                format!(
                    "Execute function and store result as '{}'",
                    clause.result_variable
                )
            };
            clause_descriptions.push(description);
        }

        let selected_index = context
            .runtime()
            .engine()
            .select(&context, &clause_descriptions)
            .await?;

        let selected_clause = &self.clauses[selected_index];

        if let Some(name) = selected_clause.expression_to_run.name() {
            info!(
                selected_function = %name,
                clause_index = selected_index,
                "select_clause"
            );
        }

        let select_context = Arc::new(Context::create_child(
            context.clone(),
            false,
            context.runtime_rc(),
        ));
        let result = selected_clause
            .expression_to_run
            .evaluate(select_context.clone())
            .await?;
        select_context.declare_variable(selected_clause.result_variable.clone(), result);

        selected_clause
            .expression_next
            .evaluate(select_context)
            .await
    }

    fn return_type(&self) -> Type {
        Type::unit()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Expression> {
        Box::new(SelectExpr {
            clauses: self
                .clauses
                .iter()
                .map(|clause| SelectClauseExpr {
                    expression_to_run: clause.expression_to_run.clone_box(),
                    result_variable: clause.result_variable.clone(),
                    expression_next: clause.expression_next.clone_box(),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompilationUnit;
    use crate::expressions::{StringLiteralExpr, VariableExpr};
    use crate::runtime::Runtime;
    use std::rc::Rc;

    fn test_runtime() -> Runtime {
        let program = CompilationUnit::from_string("fn main(): () {}".to_string());
        Runtime::builder(program).build()
    }

    #[tokio::test]
    async fn test_empty_select() {
        let select_expr = SelectExpr { clauses: vec![] };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = select_expr.evaluate(context).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("must have at least one clause")
        );
    }

    #[tokio::test]
    async fn test_single_clause_select() {
        let clause = SelectClauseExpr {
            expression_to_run: Box::new(StringLiteralExpr {
                value: "test".to_string(),
            }),
            result_variable: "result".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "result".to_string(),
            }),
        };

        let select_expr = SelectExpr {
            clauses: vec![clause],
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = select_expr.evaluate(context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "test"),
            _ => panic!("Expected string result"),
        }
    }

    #[test]
    fn test_select_clone() {
        let clause = SelectClauseExpr {
            expression_to_run: Box::new(StringLiteralExpr {
                value: "test".to_string(),
            }),
            result_variable: "result".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "result".to_string(),
            }),
        };

        let select_expr = SelectExpr {
            clauses: vec![clause],
        };

        let cloned = select_expr.clone_box();
        assert!(cloned.as_any().downcast_ref::<SelectExpr>().is_some());
    }

    #[tokio::test]
    async fn test_variable_assignment_in_select() {
        let clause = SelectClauseExpr {
            expression_to_run: Box::new(StringLiteralExpr {
                value: "assigned_value".to_string(),
            }),
            result_variable: "my_var".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "my_var".to_string(),
            }),
        };

        let select_expr = SelectExpr {
            clauses: vec![clause],
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));
        let result = select_expr.evaluate(context.clone()).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "assigned_value"),
            _ => panic!("Expected string result"),
        }

        assert!(context.get_variable("my_var").is_none());
    }

    #[tokio::test]
    async fn test_select_scope_isolation() {
        let clause = SelectClauseExpr {
            expression_to_run: Box::new(StringLiteralExpr {
                value: "scoped_value".to_string(),
            }),
            result_variable: "scoped_var".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "scoped_var".to_string(),
            }),
        };

        let select_expr = SelectExpr {
            clauses: vec![clause],
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        context.declare_variable(
            "outer_var".to_string(),
            ExprResult::String("outer_value".to_string()),
        );

        let result = select_expr.evaluate(context.clone()).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "scoped_value"),
            _ => panic!("Expected string result"),
        }

        assert_eq!(
            context.get_variable("outer_var").unwrap(),
            ExprResult::String("outer_value".to_string())
        );

        assert!(context.get_variable("scoped_var").is_none());
    }

    #[test]
    fn test_select_uses_documentation_in_descriptions() {
        use crate::expressions::FunctionExpr;
        use crate::types::Type;

        let function_with_docs = FunctionExpr {
            name: "documented_function".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "result".to_string(),
            })],
            documentation: Some("This function does something useful".to_string()),
        };

        let function_without_docs = StringLiteralExpr {
            value: "no_docs".to_string(),
        };

        let clause_with_docs = SelectClauseExpr {
            expression_to_run: Box::new(function_with_docs),
            result_variable: "documented_result".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "documented_result".to_string(),
            }),
        };

        let clause_without_docs = SelectClauseExpr {
            expression_to_run: Box::new(function_without_docs),
            result_variable: "undocumented_result".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "undocumented_result".to_string(),
            }),
        };

        assert_eq!(
            clause_with_docs.expression_to_run.documentation(),
            Some("This function does something useful")
        );

        assert_eq!(clause_without_docs.expression_to_run.documentation(), None);
    }

    #[tokio::test]
    async fn test_select_with_documented_function_integration() {
        use crate::expressions::FunctionExpr;
        use crate::runtime::Runtime;
        use crate::types::Type;
        use std::rc::Rc;

        let documented_function = FunctionExpr {
            name: "calculate_result".to_string(),
            parameters: vec![],
            return_type: Type::string(),
            body: vec![Box::new(StringLiteralExpr {
                value: "calculated_value".to_string(),
            })],
            documentation: Some("Calculates an important mathematical result".to_string()),
        };

        let clause = SelectClauseExpr {
            expression_to_run: Box::new(documented_function),
            result_variable: "calc_result".to_string(),
            expression_next: Box::new(VariableExpr {
                name: "calc_result".to_string(),
            }),
        };

        let select_expr = SelectExpr {
            clauses: vec![clause],
        };

        let runtime = Rc::new(test_runtime());
        let context = Arc::new(Context::with_runtime(runtime));

        let result = select_expr.evaluate(context).await.unwrap();

        match result {
            ExprResult::String(s) => assert_eq!(s, "calculated_value"),
            _ => panic!("Expected string result"),
        }
    }
}
