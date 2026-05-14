use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{Array, BinaryArray, StringArray, StructArray};
use arrow::datatypes::{DataType, Field, Fields};

use super::{RuntimeValue, RuntimeValueFactory};
use crate::expression::ExpressionValue;

#[derive(Debug, Clone)]
pub struct ImageValue {
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct AudioValue {
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct LinkValue {
    pub uri: String,
    pub name: Option<String>,
}

impl RuntimeValue for ImageValue {
    fn type_name(&self) -> &str {
        "Image"
    }

    fn format_for_llm(&self) -> String {
        format!("[Image: {}]", self.mime_type)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<ImageValue>()
            .map(|v| v.mime_type == self.mime_type && v.data == self.data)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        let fields = Fields::from(vec![
            Field::new("mime_type", DataType::Utf8, false)
                .with_metadata(HashMap::from([("kind".to_string(), "image".to_string())])),
            Field::new("data", DataType::Binary, false),
        ]);
        let mime_array = Arc::new(StringArray::from(vec![self.mime_type.clone()]));
        let data_array = Arc::new(BinaryArray::from(vec![self.data.as_slice()]));
        Arc::new(StructArray::new(
            fields,
            vec![mime_array as Arc<dyn Array>, data_array as Arc<dyn Array>],
            None,
        ))
    }
}

impl RuntimeValue for AudioValue {
    fn type_name(&self) -> &str {
        "Audio"
    }

    fn format_for_llm(&self) -> String {
        format!("[Audio: {}]", self.mime_type)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<AudioValue>()
            .map(|v| v.mime_type == self.mime_type && v.data == self.data)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        let fields = Fields::from(vec![
            Field::new("mime_type", DataType::Utf8, false)
                .with_metadata(HashMap::from([("kind".to_string(), "audio".to_string())])),
            Field::new("data", DataType::Binary, false),
        ]);
        let mime_array = Arc::new(StringArray::from(vec![self.mime_type.clone()]));
        let data_array = Arc::new(BinaryArray::from(vec![self.data.as_slice()]));
        Arc::new(StructArray::new(
            fields,
            vec![mime_array as Arc<dyn Array>, data_array as Arc<dyn Array>],
            None,
        ))
    }
}

impl RuntimeValue for LinkValue {
    fn type_name(&self) -> &str {
        "Link"
    }

    fn format_for_llm(&self) -> String {
        match &self.name {
            None => self.uri.clone(),
            Some(name) => format!("{} ({})", name, self.uri),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<LinkValue>()
            .map(|v| v.uri == self.uri && v.name == self.name)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        let fields = Fields::from(vec![
            Field::new("uri", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, true),
        ]);
        let uri_array = Arc::new(StringArray::from(vec![self.uri.clone()]));
        let name_array = Arc::new(StringArray::from(vec![self.name.clone()]));
        Arc::new(StructArray::new(
            fields,
            vec![uri_array as Arc<dyn Array>, name_array as Arc<dyn Array>],
            None,
        ))
    }
}

#[derive(Debug)]
pub struct ImageValueFactory;

impl RuntimeValueFactory for ImageValueFactory {
    fn type_name(&self) -> &str {
        "Image"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(ImageValue {
            mime_type: String::new(),
            data: vec![],
        })
    }
}

#[derive(Debug)]
pub struct AudioValueFactory;

impl RuntimeValueFactory for AudioValueFactory {
    fn type_name(&self) -> &str {
        "Audio"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(AudioValue {
            mime_type: String::new(),
            data: vec![],
        })
    }
}

#[derive(Debug)]
pub struct LinkValueFactory;

impl RuntimeValueFactory for LinkValueFactory {
    fn type_name(&self) -> &str {
        "Link"
    }

    fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
        Arc::new(LinkValue {
            uri: String::new(),
            name: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_to_arrow_has_correct_schema() {
        let v = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![1, 2, 3],
        };
        let arr = v.to_arrow();
        let struct_arr = arr.as_any().downcast_ref::<StructArray>().unwrap();
        let fields = struct_arr.fields();
        assert_eq!(fields[0].name(), "mime_type");
        assert_eq!(fields[0].data_type(), &DataType::Utf8);
        assert_eq!(
            fields[0].metadata().get("kind").map(|s| s.as_str()),
            Some("image")
        );
        assert_eq!(fields[1].name(), "data");
        assert_eq!(fields[1].data_type(), &DataType::Binary);
    }

    #[test]
    fn audio_to_arrow_has_correct_schema() {
        let v = AudioValue {
            mime_type: "audio/mp3".to_string(),
            data: vec![4, 5, 6],
        };
        let arr = v.to_arrow();
        let struct_arr = arr.as_any().downcast_ref::<StructArray>().unwrap();
        let fields = struct_arr.fields();
        assert_eq!(fields[0].name(), "mime_type");
        assert_eq!(fields[0].data_type(), &DataType::Utf8);
        assert_eq!(
            fields[0].metadata().get("kind").map(|s| s.as_str()),
            Some("audio")
        );
        assert_eq!(fields[1].name(), "data");
        assert_eq!(fields[1].data_type(), &DataType::Binary);
    }

    #[test]
    fn link_to_arrow_has_correct_schema() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: Some("Example".to_string()),
        };
        let arr = v.to_arrow();
        let struct_arr = arr.as_any().downcast_ref::<StructArray>().unwrap();
        let fields = struct_arr.fields();
        assert_eq!(fields[0].name(), "uri");
        assert_eq!(fields[0].data_type(), &DataType::Utf8);
        assert!(!fields[0].is_nullable());
        assert!(fields[0].metadata().get("kind").is_none());
        assert_eq!(fields[1].name(), "name");
        assert_eq!(fields[1].data_type(), &DataType::Utf8);
        assert!(fields[1].is_nullable());
    }

    #[test]
    fn link_to_arrow_with_none_name_is_null() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        let arr = v.to_arrow();
        let struct_arr = arr.as_any().downcast_ref::<StructArray>().unwrap();
        let name_col = struct_arr
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert!(name_col.is_null(0));
    }

    #[test]
    fn image_type_name() {
        let v = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![],
        };
        assert_eq!(v.type_name(), "Image");
    }

    #[test]
    fn audio_type_name() {
        let v = AudioValue {
            mime_type: "audio/mp3".to_string(),
            data: vec![],
        };
        assert_eq!(v.type_name(), "Audio");
    }

    #[test]
    fn link_type_name() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        assert_eq!(v.type_name(), "Link");
    }

    #[test]
    fn image_eq_same_value() {
        let a = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![1, 2, 3],
        };
        let b = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![1, 2, 3],
        };
        assert!(RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn image_not_eq_different_mime() {
        let a = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![1, 2, 3],
        };
        let b = ImageValue {
            mime_type: "image/jpeg".to_string(),
            data: vec![1, 2, 3],
        };
        assert!(!RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn image_not_eq_different_data() {
        let a = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![1, 2, 3],
        };
        let b = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![4, 5, 6],
        };
        assert!(!RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn audio_eq_same_value() {
        let a = AudioValue {
            mime_type: "audio/mp3".to_string(),
            data: vec![10, 20],
        };
        let b = AudioValue {
            mime_type: "audio/mp3".to_string(),
            data: vec![10, 20],
        };
        assert!(RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn link_eq_same_uri() {
        let a = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        let b = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        assert!(RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn link_not_eq_different_uri() {
        let a = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        let b = LinkValue {
            uri: "https://other.com".to_string(),
            name: None,
        };
        assert!(!RuntimeValue::eq(&a, b.as_any()));
    }

    #[test]
    fn image_format_for_llm_includes_mime_type() {
        let v = ImageValue {
            mime_type: "image/png".to_string(),
            data: vec![],
        };
        assert_eq!(v.format_for_llm(), "[Image: image/png]");
    }

    #[test]
    fn audio_format_for_llm_includes_mime_type() {
        let v = AudioValue {
            mime_type: "audio/mp3".to_string(),
            data: vec![],
        };
        assert_eq!(v.format_for_llm(), "[Audio: audio/mp3]");
    }

    #[test]
    fn link_format_for_llm_is_uri() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: None,
        };
        assert_eq!(v.format_for_llm(), "https://example.com");
    }

    #[test]
    fn link_name_is_optional() {
        let v = LinkValue {
            uri: "https://example.com".to_string(),
            name: Some("Example".to_string()),
        };
        assert_eq!(v.format_for_llm(), "Example (https://example.com)");
    }
}
