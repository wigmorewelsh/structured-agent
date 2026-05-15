use std::any::Any;
use std::sync::Arc;

use arrow::array::{
    Array, BinaryArray, BooleanArray, Int64Array, ListArray, StringArray, StructArray, UnionArray,
};
use arrow::datatypes::DataType;

use crate::expression::ExpressionValue;

mod list;
mod list_iterator;
mod media;
mod primitives;
mod struct_value;

pub use list::{ListValue, ListValueFactory};
pub use list_iterator::{ListIteratorValue, ListIteratorValueFactory};
pub use media::{
    AudioValue, AudioValueFactory, ImageValue, ImageValueFactory, LinkValue, LinkValueFactory,
};
pub use primitives::{
    BooleanValue, BooleanValueFactory, IntValue, IntValueFactory, StringValue, StringValueFactory,
    UnitValue, UnitValueFactory,
};
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
    fn generic_params(&self) -> Vec<String> {
        vec![]
    }
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

fn is_image_struct_array(arr: &StructArray) -> bool {
    arr.fields()
        .first()
        .map(|f| {
            f.metadata()
                .get("kind")
                .map(|v| v == "image")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn is_audio_struct_array(arr: &StructArray) -> bool {
    arr.fields()
        .first()
        .map(|f| {
            f.metadata()
                .get("kind")
                .map(|v| v == "audio")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn is_link_struct_array(arr: &StructArray) -> bool {
    let fields = arr.fields();
    fields.len() == 2
        && fields[0].name() == "uri"
        && fields[0].data_type() == &DataType::Utf8
        && fields[1].name() == "name"
        && fields[1].data_type() == &DataType::Utf8
}

pub fn arrow_col_to_expression(col: Arc<dyn Array>) -> ExpressionValue {
    match col.data_type() {
        DataType::Null => ExpressionValue::unit(),
        DataType::Utf8 => {
            let arr = col.as_any().downcast_ref::<StringArray>().expect("utf8");
            ExpressionValue::string(arr.value(0).to_string())
        }
        DataType::Boolean => {
            let arr = col.as_any().downcast_ref::<BooleanArray>().expect("bool");
            ExpressionValue::boolean(arr.value(0))
        }
        DataType::Int64 => {
            let arr = col.as_any().downcast_ref::<Int64Array>().expect("int64");
            ExpressionValue::integer(arr.value(0))
        }
        DataType::List(_) => {
            let arr = col.as_any().downcast_ref::<ListArray>().expect("list");
            ExpressionValue::list(Arc::new(arr.clone()))
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
                ExpressionValue::metadata(name, documentation)
            } else if is_image_struct_array(arr) {
                let mime_type = arr
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("mime_type")
                    .value(0)
                    .to_string();
                let data = arr
                    .column(1)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .expect("data")
                    .value(0)
                    .to_vec();
                ExpressionValue::image(mime_type, data)
            } else if is_audio_struct_array(arr) {
                let mime_type = arr
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("mime_type")
                    .value(0)
                    .to_string();
                let data = arr
                    .column(1)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .expect("data")
                    .value(0)
                    .to_vec();
                ExpressionValue::audio(mime_type, data)
            } else if is_link_struct_array(arr) {
                let uri = arr
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("uri")
                    .value(0)
                    .to_string();
                let name_col = arr
                    .column(1)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("name");
                let name = if name_col.is_null(0) {
                    None
                } else {
                    Some(name_col.value(0).to_string())
                };
                ExpressionValue::link(uri, name)
            } else {
                ExpressionValue::struct_from_array(Arc::new(arr.clone()))
            }
        }
        DataType::Union(_, _) => {
            let union = col
                .as_any()
                .downcast_ref::<UnionArray>()
                .expect("union array");
            arrow_col_to_expression(union.value(0))
        }
        _ => ExpressionValue::unit(),
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime_value::{AudioValue, ImageValue, LinkValue, RuntimeValue};

    use super::arrow_col_to_expression;

    #[test]
    fn arrow_col_to_expression_roundtrip_image() {
        let v = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![10, 20, 30],
        };
        let arr = v.to_arrow();
        let result = arrow_col_to_expression(arr);
        let recovered = result.as_image().unwrap();
        assert_eq!(recovered.mime_type, "image/png");
        assert_eq!(recovered.data, vec![10, 20, 30]);
    }

    #[test]
    fn arrow_col_to_expression_roundtrip_audio() {
        let v = AudioValue {
            mime_type: "audio/mp3".to_string(),
            data: vec![40, 50, 60],
        };
        let arr = v.to_arrow();
        let result = arrow_col_to_expression(arr);
        let recovered = result.as_audio().unwrap();
        assert_eq!(recovered.mime_type, "audio/mp3");
        assert_eq!(recovered.data, vec![40, 50, 60]);
    }

    #[test]
    fn arrow_col_to_expression_roundtrip_link_with_name() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: Some("Example".to_string()),
        };
        let arr = v.to_arrow();
        let result = arrow_col_to_expression(arr);
        let recovered = result.as_link().unwrap();
        assert_eq!(recovered.uri, "https://example.com");
        assert_eq!(recovered.name, Some("Example".to_string()));
    }

    #[test]
    fn arrow_col_to_expression_roundtrip_link_without_name() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        let arr = v.to_arrow();
        let result = arrow_col_to_expression(arr);
        let recovered = result.as_link().unwrap();
        assert_eq!(recovered.uri, "https://example.com");
        assert_eq!(recovered.name, None);
    }
}
