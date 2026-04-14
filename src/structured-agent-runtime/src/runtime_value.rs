use std::any::Any;
use std::sync::Arc;

use arrow::array::{Array, NullArray};

use crate::expression::ExpressionValue;

pub trait RuntimeValue: std::fmt::Debug + Send + Sync {
    fn type_name(&self) -> &str;
    fn format_for_llm(&self) -> String;
    fn as_any(&self) -> &dyn Any;
    fn eq(&self, other: &dyn Any) -> bool;
    fn to_arrow(&self) -> Arc<dyn Array>;
}

pub trait RuntimeValueFactory: std::fmt::Debug + Send + Sync {
    fn type_name(&self) -> &str;
    fn construct(&self, args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue>;
}

#[derive(Debug)]
pub struct UnitValue;

impl RuntimeValue for UnitValue {
    fn type_name(&self) -> &str {
        "Unit"
    }

    fn format_for_llm(&self) -> String {
        "()".to_string()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<UnitValue>().is_some()
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        Arc::new(NullArray::new(1))
    }
}
