use std::any::Any;
use std::sync::Arc;

use arrow::array::{Array, BooleanArray, Int64Array, ListArray, StringArray, StructArray, UnionArray};
use arrow::datatypes::DataType;

use crate::expression::ExpressionValue;

mod list;
mod option;
mod primitives;
mod struct_value;

pub use list::{ListValue, ListValueFactory};
pub use option::{OptionValue, OptionValueFactory};
pub use primitives::{BooleanValue, IntValue, StringValue, UnitValue};
pub use struct_value::{MetadataValue, StructValue};

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

pub fn is_metadata_struct_array(arr: &StructArray) -> bool {
    arr.fields()
        .first()
        .map(|f| {
            f.metadata()
                .get("kind")
                .map(|v| v == "metadata")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn arrow_col_to_expression(col: Arc<dyn Array>) -> ExpressionValue {
    match col.data_type() {
        DataType::Null => ExpressionValue::unit(),
        DataType::Utf8 => {
            let arr = col.as_any().downcast_ref::<StringArray>().expect("utf8");
            ExpressionValue::Dynamic(Arc::new(StringValue(arr.value(0).to_string())))
        }
        DataType::Boolean => {
            let arr = col.as_any().downcast_ref::<BooleanArray>().expect("bool");
            ExpressionValue::Dynamic(Arc::new(BooleanValue(arr.value(0))))
        }
        DataType::Int64 => {
            let arr = col.as_any().downcast_ref::<Int64Array>().expect("int64");
            ExpressionValue::Dynamic(Arc::new(IntValue(arr.value(0))))
        }
        DataType::List(_) => {
            let arr = col.as_any().downcast_ref::<ListArray>().expect("list");
            ExpressionValue::Dynamic(Arc::new(ListValue::new(Arc::new(arr.clone()))))
        }
        DataType::Struct(_) => {
            let arr = col.as_any().downcast_ref::<StructArray>().expect("struct");
            if is_metadata_struct_array(arr) {
                let name_arr = arr
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("name");
                let doc_arr = arr
                    .column(1)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("doc");
                let name = name_arr.value(0).to_string();
                let documentation = if doc_arr.is_null(0) {
                    None
                } else {
                    Some(doc_arr.value(0).to_string())
                };
                ExpressionValue::Dynamic(Arc::new(MetadataValue { name, documentation }))
            } else {
                ExpressionValue::Dynamic(Arc::new(StructValue {
                    struct_array: Arc::new(arr.clone()),
                }))
            }
        }
        DataType::Union(_, _) => {
            let arr = col.as_any().downcast_ref::<UnionArray>().expect("union");
            ExpressionValue::Dynamic(Arc::new(OptionValue::from_union(Arc::new(arr.clone()))))
        }
        _ => ExpressionValue::unit(),
    }
}
