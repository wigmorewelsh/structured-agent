use std::any::Any;
use std::sync::Arc;

use arrow::array::{Array, ListArray, NullArray, UnionArray};
use arrow::buffer::{OffsetBuffer, ScalarBuffer};
use arrow::compute::concat;
use arrow::datatypes::{DataType, Field, FieldRef, UnionFields};

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

#[derive(Debug)]
pub struct ListValue {
    list: Arc<ListArray>,
}

impl ListValue {
    pub fn new(list: Arc<ListArray>) -> Self {
        Self { list }
    }

    pub fn list_array(&self) -> &ListArray {
        &self.list
    }

    pub fn from_elements(elements: Vec<ExpressionValue>) -> Result<Self, String> {
        if elements.is_empty() {
            let child = arrow::array::new_empty_array(&DataType::Utf8);
            let field = Arc::new(Field::new("item", DataType::Utf8, true)) as FieldRef;
            let offsets = OffsetBuffer::new(vec![0i32, 0i32].into());
            let list_array =
                ListArray::try_new(field, offsets, child, None).map_err(|e| e.to_string())?;
            return Ok(Self::new(Arc::new(list_array)));
        }

        let arrays: Vec<Arc<dyn Array>> = elements.iter().map(|e| e.arrow_data()).collect();
        let refs: Vec<&dyn Array> = arrays.iter().map(|a| a.as_ref()).collect();
        let child = concat(&refs).map_err(|e| e.to_string())?;
        let field = Arc::new(Field::new("item", child.data_type().clone(), true)) as FieldRef;
        let offsets = OffsetBuffer::new(vec![0i32, child.len() as i32].into());
        let list_array =
            ListArray::try_new(field, offsets, child, None).map_err(|e| e.to_string())?;
        Ok(Self::new(Arc::new(list_array)))
    }
}

impl RuntimeValue for ListValue {
    fn type_name(&self) -> &str {
        "List"
    }

    fn format_for_llm(&self) -> String {
        ExpressionValue::Arrow(self.list.clone() as Arc<dyn Array>).format_for_llm()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<ListValue>()
            .map(|v| v.list.as_ref() == self.list.as_ref())
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        self.list.clone() as Arc<dyn Array>
    }
}

#[derive(Debug)]
pub struct OptionValue {
    union: Arc<UnionArray>,
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

    pub fn inner(&self) -> Option<ExpressionValue> {
        let union: &UnionArray = &self.union;
        match union.type_id(0) {
            0 => None,
            1 => Some(ExpressionValue::Arrow(union.value(0))),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ListValueFactory;

impl RuntimeValueFactory for ListValueFactory {
    fn type_name(&self) -> &str {
        "List"
    }

    fn construct(&self, args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(ListValue::from_elements(args).expect("ListValueFactory::construct failed"))
    }
}

#[derive(Debug)]
pub struct OptionValueFactory;

impl RuntimeValueFactory for OptionValueFactory {
    fn type_name(&self) -> &str {
        "Option"
    }

    fn construct(&self, args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        match args.into_iter().next() {
            Some(v) => Arc::new(OptionValue::some(v.arrow_data())),
            None => Arc::new(OptionValue::none()),
        }
    }
}

impl RuntimeValue for OptionValue {
    fn type_name(&self) -> &str {
        "Option"
    }

    fn format_for_llm(&self) -> String {
        ExpressionValue::Arrow(self.union.clone() as Arc<dyn Array>).format_for_llm()
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
