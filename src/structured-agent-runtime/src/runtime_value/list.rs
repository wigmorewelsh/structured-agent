use std::any::Any;
use std::sync::Arc;

use arrow::array::{Array, ListArray};
use arrow::buffer::OffsetBuffer;
use arrow::compute::concat;
use arrow::datatypes::{DataType, Field, FieldRef};

use crate::expression::ExpressionValue;
use crate::runtime_value::{RuntimeValue, RuntimeValueFactory, arrow_col_to_expression};

#[derive(Debug)]
pub struct ListValue {
    list: Arc<ListArray>,
}

impl ListValue {
    pub fn new(list: Arc<ListArray>) -> Self {
        Self { list }
    }

    pub fn list_arc(&self) -> Arc<ListArray> {
        self.list.clone()
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

        let arrays: Vec<Arc<dyn Array>> = elements.iter().map(|e| e.to_arrow()).collect();
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
        if self.list.is_empty() {
            return "[]".to_string();
        }
        let values = self.list.value(0);
        let items: Vec<String> = (0..values.len())
            .map(|i| arrow_col_to_expression(values.slice(i, 1)).format_for_llm())
            .collect();
        format!("[{}]", items.join(", "))
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
pub struct ListValueFactory;

impl RuntimeValueFactory for ListValueFactory {
    fn type_name(&self) -> &str {
        "List"
    }

    fn construct(&self, args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(ListValue::from_elements(args).expect("ListValueFactory::construct failed"))
    }
}
