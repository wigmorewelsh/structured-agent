use std::any::Any;
use std::ops::Deref;
use std::sync::Arc;

use arrow::array::{Array, BooleanArray, Int64Array, NullArray, StringArray};

use super::{RuntimeValue, RuntimeValueFactory};
use crate::expression::ExpressionValue;

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct StringValue(pub String);

impl Deref for StringValue {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StringValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for StringValue {
    fn from(s: String) -> Self {
        StringValue(s)
    }
}

impl From<&str> for StringValue {
    fn from(s: &str) -> Self {
        StringValue(s.to_string())
    }
}

impl From<StringValue> for String {
    fn from(v: StringValue) -> Self {
        v.0
    }
}

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

#[derive(Debug, Clone)]
pub struct IntValue(pub i64);

impl Deref for IntValue {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}

impl std::fmt::Display for IntValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<i64> for IntValue {
    fn from(n: i64) -> Self {
        IntValue(n)
    }
}

impl From<IntValue> for i64 {
    fn from(v: IntValue) -> Self {
        v.0
    }
}

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

#[derive(Debug, Clone)]
pub struct BooleanValue(pub bool);

impl Deref for BooleanValue {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.0
    }
}

impl std::fmt::Display for BooleanValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<bool> for BooleanValue {
    fn from(b: bool) -> Self {
        BooleanValue(b)
    }
}

impl From<BooleanValue> for bool {
    fn from(v: BooleanValue) -> Self {
        v.0
    }
}

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

#[derive(Debug)]
pub struct UnitValueFactory;

impl RuntimeValueFactory for UnitValueFactory {
    fn type_name(&self) -> &str {
        "Unit"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(UnitValue)
    }
}

#[derive(Debug)]
pub struct BooleanValueFactory;

impl RuntimeValueFactory for BooleanValueFactory {
    fn type_name(&self) -> &str {
        "Boolean"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(BooleanValue(false))
    }
}

#[derive(Debug)]
pub struct StringValueFactory;

impl RuntimeValueFactory for StringValueFactory {
    fn type_name(&self) -> &str {
        "String"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(StringValue(std::string::String::new()))
    }
}

#[derive(Debug)]
pub struct IntValueFactory;

impl RuntimeValueFactory for IntValueFactory {
    fn type_name(&self) -> &str {
        "Int"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(IntValue(0))
    }
}
