use crate::runtime::{Context, ExprResult};
use crate::types::{Expression, Type};
use async_trait::async_trait;
use std::any::Any;

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
    async fn evaluate(&self, context: &mut Context) -> Result<ExprResult, String> {
        if self.clauses.is_empty() {
            return Err("Select statement must have at least one clause".to_string());
        }

        let mut clause_descriptions = Vec::new();
        for clause in &self.clauses {
            clause_descriptions.push(format!(
                "Execute function and store result as '{}'",
                clause.result_variable
            ));
        }

        let selected_index = context
            .runtime()
            .engine()
            .select(context, &clause_descriptions)
            .await?;

        let selected_clause = &self.clauses[selected_index];

        let mut select_context = context.create_child();
        let result = selected_clause
            .expression_to_run
            .evaluate(&mut select_context)
            .await?;
        select_context.set_variable(selected_clause.result_variable.clone(), result);

        selected_clause
            .expression_next
            .evaluate(&mut select_context)
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
    use crate::expressions::{StringLiteralExpr, VariableExpr};
    use crate::runtime::Runtime;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_empty_select() {
        let select_expr = SelectExpr { clauses: vec![] };

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = select_expr.evaluate(&mut context).await;

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

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = select_expr.evaluate(&mut context).await.unwrap();

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

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);
        let result = select_expr.evaluate(&mut context).await.unwrap();

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

        let runtime = Rc::new(Runtime::new());
        let mut context = Context::with_runtime(runtime);

        // Set an outer variable before the select
        context.set_variable(
            "outer_var".to_string(),
            ExprResult::String("outer_value".to_string()),
        );

        let result = select_expr.evaluate(&mut context).await.unwrap();

        // The select should return the scoped value
        match result {
            ExprResult::String(s) => assert_eq!(s, "scoped_value"),
            _ => panic!("Expected string result"),
        }

        // The outer variable should still exist
        assert_eq!(
            context.get_variable("outer_var").unwrap(),
            &ExprResult::String("outer_value".to_string())
        );

        // The scoped variable should NOT exist in the outer context
        assert!(context.get_variable("scoped_var").is_none());
    }
}
