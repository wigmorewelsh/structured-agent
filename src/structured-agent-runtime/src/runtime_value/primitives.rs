use std::any::Any;
use std::sync::Arc;

use arrow::array::{Array, BooleanArray, Int64Array, NullArray, StringArray};

use super::RuntimeValue;

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

#[derive(Debug)]
pub struct StringValue(pub String);

impl RuntimeValue for StringValue {
    fn type_name(&self) -> &str {
        "String"
    }

    fn format_for_llm(&self) -> String {
        self.0.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<StringValue>()
            .map(|v| v.0 == self.0)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        Arc::new(StringArray::from(vec![self.0.clone()]))
    }
}

#[derive(Debug)]
pub struct IntValue(pub i64);

impl RuntimeValue for IntValue {
    fn type_name(&self) -> &str {
        "Int"
    }

    fn format_for_llm(&self) -> String {
        self.0.to_string()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<IntValue>()
            .map(|v| v.0 == self.0)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        Arc::new(Int64Array::from(vec![self.0]))
    }
}

#[derive(Debug)]
pub struct BooleanValue(pub bool);

impl RuntimeValue for BooleanValue {
    fn type_name(&self) -> &str {
        "Boolean"
    }

    fn format_for_llm(&self) -> String {
        self.0.to_string()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<BooleanValue>()
            .map(|v| v.0 == self.0)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        Arc::new(BooleanArray::from(vec![self.0]))
    }
}
