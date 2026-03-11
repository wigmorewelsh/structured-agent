use arrow::array::{Array, BooleanArray, ListArray, NullArray, StringArray, StructArray};
use arrow::datatypes::{DataType, Field, Fields};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionResult {
    pub name: Option<String>,
    pub params: Option<Vec<ExpressionParameter>>,
    pub value: ExpressionValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionParameter {
    pub name: String,
    pub value: ExpressionValue,
}

impl ExpressionParameter {
    pub fn new(name: String, value: ExpressionValue) -> Self {
        Self { name, value }
    }
}

// was ExprResult is now ExpressionValue
#[derive(Debug, Clone)]
pub struct ExpressionValue {
    data: Arc<dyn Array>,
}

impl PartialEq for ExpressionValue {
    fn eq(&self, other: &Self) -> bool {
        self.data.as_ref() == other.data.as_ref()
    }
}

impl ExpressionResult {
    pub fn new(value: ExpressionValue) -> Self {
        Self {
            name: None,
            params: None,
            value,
        }
    }

    pub fn with_params(value: ExpressionValue, params: Vec<ExpressionParameter>) -> Self {
        Self {
            name: None,
            params: Some(params),
            value,
        }
    }

    pub fn with_name(value: ExpressionValue, name: String) -> Self {
        Self {
            name: Some(name),
            params: None,
            value,
        }
    }

    pub fn with_name_and_params(
        value: ExpressionValue,
        name: String,
        params: Vec<ExpressionParameter>,
    ) -> Self {
        Self {
            name: Some(name),
            params: Some(params),
            value,
        }
    }
}

impl ExpressionValue {
    // Constructors
    pub fn unit() -> Self {
        Self {
            data: Arc::new(NullArray::new(1)),
        }
    }

    pub fn string(s: impl Into<String>) -> Self {
        Self {
            data: Arc::new(StringArray::from(vec![s.into()])),
        }
    }

    pub fn boolean(b: bool) -> Self {
        Self {
            data: Arc::new(BooleanArray::from(vec![b])),
        }
    }

    pub fn list(arr: Arc<ListArray>) -> Self {
        Self { data: arr }
    }

    pub fn metadata(name: impl Into<String>, documentation: Option<String>) -> Self {
        let name_str = name.into();

        // Create fields for the struct
        let fields = Fields::from(vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("documentation", DataType::Utf8, true),
        ]);

        // Create the arrays for each field
        let name_array = Arc::new(StringArray::from(vec![name_str]));
        let doc_array = Arc::new(StringArray::from(vec![documentation]));

        // Create the struct array
        let struct_array = StructArray::new(
            fields,
            vec![name_array as Arc<dyn Array>, doc_array as Arc<dyn Array>],
            None,
        );

        Self {
            data: Arc::new(struct_array),
        }
    }

    // Helper for scalar extraction
    fn downcast_scalar<T: Array + 'static>(&self) -> Result<&T, String> {
        self.data
            .as_any()
            .downcast_ref::<T>()
            .filter(|arr| arr.len() == 1)
            .ok_or_else(|| format!("Expected scalar {}", std::any::type_name::<T>()))
    }

    // Accessors
    pub fn as_string(&self) -> Result<&str, String> {
        self.downcast_scalar::<StringArray>().and_then(|arr| {
            if arr.is_null(0) {
                Err("String array is null".to_string())
            } else {
                Ok(arr.value(0))
            }
        })
    }

    pub fn as_boolean(&self) -> Result<bool, String> {
        self.downcast_scalar::<BooleanArray>().and_then(|arr| {
            if arr.is_null(0) {
                Err("Boolean array is null".to_string())
            } else {
                Ok(arr.value(0))
            }
        })
    }

    pub fn as_list(&self) -> Result<&Arc<ListArray>, String> {
        if self.data.as_any().is::<ListArray>() {
            unsafe {
                Ok(std::mem::transmute::<&Arc<dyn Array>, &Arc<ListArray>>(
                    &self.data,
                ))
            }
        } else {
            Err("Expected list".to_string())
        }
    }

    pub fn type_name(&self) -> &str {
        use arrow::datatypes::DataType;
        match self.data.data_type() {
            DataType::Null => "Unit",
            DataType::Utf8 => "String",
            DataType::Boolean => "Boolean",
            DataType::List(_) => "List",
            DataType::Struct(_) => "Metadata",
            _ => "Unknown",
        }
    }

    pub fn as_metadata(&self) -> Result<(String, Option<String>), String> {
        if let Some(struct_array) = self.data.as_any().downcast_ref::<StructArray>() {
            if struct_array.len() != 1 {
                return Err("Expected single metadata value".to_string());
            }

            let name_array = struct_array
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or("Expected name field to be string")?;

            let doc_array = struct_array
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or("Expected documentation field to be string")?;

            let name = name_array.value(0).to_string();
            let documentation = if doc_array.is_null(0) {
                None
            } else {
                Some(doc_array.value(0).to_string())
            };

            Ok((name, documentation))
        } else {
            Err("Expected metadata struct".to_string())
        }
    }

    pub fn value_string(&self) -> String {
        if self.data.as_any().is::<NullArray>() {
            "()".to_string()
        } else if let Ok(s) = self.as_string() {
            s.to_string()
        } else if let Ok(b) = self.as_boolean() {
            b.to_string()
        } else if let Ok(list) = self.as_list() {
            format!("{:?}", list)
        } else if let Ok((name, documentation)) = self.as_metadata() {
            if let Some(doc) = documentation {
                format!("Metadata({}, \"{}\")", name, doc)
            } else {
                format!("Metadata({})", name)
            }
        } else {
            format!("{:?}", self.data)
        }
    }

    pub fn format_for_llm(&self) -> String {
        if self.data.as_any().is::<NullArray>() {
            "()".to_string()
        } else if let Ok(s) = self.as_string() {
            s.to_string()
        } else if let Ok(b) = self.as_boolean() {
            b.to_string()
        } else if let Ok(list) = self.as_list() {
            if list.len() == 0 {
                "[]".to_string()
            } else {
                let values = list.value(0);
                if let Some(string_array) = values.as_any().downcast_ref::<StringArray>() {
                    let items: Vec<String> = (0..string_array.len())
                        .map(|i| format!("\"{}\"", string_array.value(i)))
                        .collect();
                    format!("[{}]", items.join(", "))
                } else {
                    "[]".to_string()
                }
            }
        } else if let Ok((name, documentation)) = self.as_metadata() {
            if let Some(doc) = documentation {
                format!("{}: {}", name, doc)
            } else {
                name
            }
        } else {
            self.value_string()
        }
    }
}

// From implementations for ergonomic construction
impl From<String> for ExpressionValue {
    fn from(s: String) -> Self {
        Self::string(s)
    }
}

impl From<&str> for ExpressionValue {
    fn from(s: &str) -> Self {
        Self::string(s)
    }
}

impl From<bool> for ExpressionValue {
    fn from(b: bool) -> Self {
        Self::boolean(b)
    }
}

impl From<Arc<ListArray>> for ExpressionValue {
    fn from(arr: Arc<ListArray>) -> Self {
        Self::list(arr)
    }
}

impl std::fmt::Display for ExpressionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value_string())
    }
}
