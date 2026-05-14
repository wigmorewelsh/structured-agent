use std::any::Any;
use std::sync::Arc;

use arrow::array::{Array, NullArray, UnionArray};
use arrow::buffer::ScalarBuffer;
use arrow::datatypes::{DataType, Field, UnionFields};

use crate::expression::ExpressionValue;

use super::{RuntimeValue, RuntimeValueFactory, arrow_col_to_expression};

#[derive(Debug)]
pub struct OptionValue {
    pub union: Arc<UnionArray>,
}

impl OptionValue {
    pub fn none() -> Self {
        let union_fields = UnionFields::try_new(
            [0_i8, 1_i8],
            [
                Field::new("none", DataType::Null, true),
                Field::new("some", DataType::Null, true),
            ],
        )
        .expect("valid option_none union fields");
        let type_ids: ScalarBuffer<i8> = [0_i8].into_iter().collect();
        let offsets: ScalarBuffer<i32> = [0_i32].into_iter().collect();
        let children: Vec<Arc<dyn Array>> =
            vec![Arc::new(NullArray::new(1)), Arc::new(NullArray::new(0))];
        let union_array = UnionArray::try_new(union_fields, type_ids, Some(offsets), children)
            .expect("valid option_none union");
        Self {
            union: Arc::new(union_array),
        }
    }

    pub fn none_with_type(some_type: DataType) -> Self {
        let union_fields = UnionFields::try_new(
            [0_i8, 1_i8],
            [
                Field::new("none", DataType::Null, true),
                Field::new("some", some_type.clone(), false),
            ],
        )
        .expect("valid option_none_with_type union fields");
        let type_ids: ScalarBuffer<i8> = [0_i8].into_iter().collect();
        let offsets: ScalarBuffer<i32> = [0_i32].into_iter().collect();
        let children: Vec<Arc<dyn Array>> = vec![
            Arc::new(NullArray::new(1)),
            arrow::array::new_empty_array(&some_type),
        ];
        let union_array = UnionArray::try_new(union_fields, type_ids, Some(offsets), children)
            .expect("valid option_none_with_type union");
        Self {
            union: Arc::new(union_array),
        }
    }

    pub fn none_utf8() -> Self {
        Self::none_with_type(DataType::Utf8)
    }

    pub fn none_boolean() -> Self {
        Self::none_with_type(DataType::Boolean)
    }

    pub fn none_int64() -> Self {
        Self::none_with_type(DataType::Int64)
    }

    pub fn some(inner: Arc<dyn Array>) -> Self {
        let inner_type = inner.data_type().clone();
        let union_fields = UnionFields::try_new(
            [0_i8, 1_i8],
            [
                Field::new("none", DataType::Null, true),
                Field::new("some", inner_type, false),
            ],
        )
        .expect("valid option_some union fields");
        let type_ids: ScalarBuffer<i8> = [1_i8].into_iter().collect();
        let offsets: ScalarBuffer<i32> = [0_i32].into_iter().collect();
        let children: Vec<Arc<dyn Array>> = vec![Arc::new(NullArray::new(0)), inner];
        let union_array = UnionArray::try_new(union_fields, type_ids, Some(offsets), children)
            .expect("valid option_some union");
        Self {
            union: Arc::new(union_array),
        }
    }

    pub fn from_union(union: Arc<UnionArray>) -> Self {
        Self { union }
    }

    pub fn inner(&self) -> Option<ExpressionValue> {
        let union: &UnionArray = &self.union;
        match union.type_id(0) {
            0 => None,
            1 => Some(arrow_col_to_expression(union.value(0))),
            _ => None,
        }
    }
}

impl RuntimeValue for OptionValue {
    fn type_name(&self) -> &str {
        "Option"
    }

    fn format_for_llm(&self) -> String {
        match self.inner() {
            None => "None".to_string(),
            Some(inner) => format!("Some({})", inner.format_for_llm()),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<OptionValue>()
            .map(|v| {
                let a: &dyn Array = self.union.as_ref();
                let b: &dyn Array = v.union.as_ref();
                a == b
            })
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        self.union.clone() as Arc<dyn Array>
    }
}

#[derive(Debug)]
pub struct OptionValueFactory;

impl RuntimeValueFactory for OptionValueFactory {
    fn type_name(&self) -> &str {
        "Option"
    }

    fn generic_params(&self) -> Vec<String> {
        vec!["T".to_string()]
    }

    fn construct(&self, args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        match args.into_iter().next() {
            Some(v) => Arc::new(OptionValue::some(v.to_arrow())),
            None => Arc::new(OptionValue::none()),
        }
    }
}
