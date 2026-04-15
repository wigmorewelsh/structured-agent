use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{Array, StringArray, StructArray};
use arrow::datatypes::{DataType, Field, Fields};

use crate::expression::ExpressionValue;

use super::arrow_col_to_expression;

#[derive(Debug)]
pub struct StructValue {
    pub struct_array: Arc<StructArray>,
}

impl StructValue {
    pub fn from_fields(fields: Vec<(&str, ExpressionValue)>) -> Self {
        let arrow_fields: Vec<Field> = fields
            .iter()
            .map(|(name, val)| Field::new(*name, val.to_arrow().data_type().clone(), true))
            .collect();
        let arrays: Vec<Arc<dyn Array>> = fields.iter().map(|(_, val)| val.to_arrow()).collect();
        let struct_array = StructArray::new(Fields::from(arrow_fields), arrays, None);
        Self {
            struct_array: Arc::new(struct_array),
        }
    }

    pub fn get_field(&self, field: &str) -> Result<ExpressionValue, String> {
        let (index, _) = self
            .struct_array
            .fields()
            .iter()
            .enumerate()
            .find(|(_, f)| f.name() == field)
            .ok_or_else(|| format!("No field '{}' in struct", field))?;
        let col = self.struct_array.column(index);
        Ok(arrow_col_to_expression(col.clone()))
    }
}

impl super::RuntimeValue for StructValue {
    fn type_name(&self) -> &str {
        "Struct"
    }

    fn format_for_llm(&self) -> String {
        let mut obj = serde_json::Map::new();
        for (i, field) in self.struct_array.fields().iter().enumerate() {
            let col = self.struct_array.column(i);
            let val = arrow_col_to_expression(col.clone());
            obj.insert(
                field.name().clone(),
                serde_json::Value::String(val.format_for_llm()),
            );
        }
        serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<StructValue>()
            .map(|v| {
                let a: &dyn Array = self.struct_array.as_ref();
                let b: &dyn Array = v.struct_array.as_ref();
                a == b
            })
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        self.struct_array.clone() as Arc<dyn Array>
    }
}

#[derive(Debug)]
pub struct MetadataValue {
    pub name: String,
    pub documentation: Option<String>,
}

impl MetadataValue {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn documentation(&self) -> Option<&str> {
        self.documentation.as_deref()
    }
}

impl super::RuntimeValue for MetadataValue {
    fn type_name(&self) -> &str {
        "Metadata"
    }

    fn format_for_llm(&self) -> String {
        match &self.documentation {
            Some(doc) => format!("{}: {}", self.name, doc),
            None => self.name.clone(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<MetadataValue>()
            .map(|v| v.name == self.name && v.documentation == self.documentation)
            .unwrap_or(false)
    }

    fn to_arrow(&self) -> Arc<dyn Array> {
        let marker: HashMap<String, String> = [("kind".to_string(), "metadata".to_string())].into();
        let fields = Fields::from(vec![
            Field::new("name", DataType::Utf8, false).with_metadata(marker),
            Field::new("documentation", DataType::Utf8, true),
        ]);
        let name_array = Arc::new(StringArray::from(vec![self.name.clone()]));
        let doc_array = Arc::new(StringArray::from(vec![self.documentation.clone()]));
        let struct_array = StructArray::new(
            fields,
            vec![name_array as Arc<dyn Array>, doc_array as Arc<dyn Array>],
            None,
        );
        Arc::new(struct_array) as Arc<dyn Array>
    }
}
