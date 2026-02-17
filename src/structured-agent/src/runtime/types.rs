use arrow::array::{Array, ListArray};
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
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionValue {
    Unit,
    String(String),
    Boolean(bool),
    List(Arc<ListArray>),
    Option(Option<Box<ExpressionValue>>),
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
    pub fn as_string(&self) -> Result<&str, String> {
        match self {
            ExpressionValue::String(s) => Ok(s),
            _ => Err("Expected string result".to_string()),
        }
    }

    pub fn as_boolean(&self) -> Result<bool, String> {
        match self {
            ExpressionValue::Boolean(b) => Ok(*b),
            _ => Err("Expected boolean result".to_string()),
        }
    }

    pub fn as_list(&self) -> Result<&Arc<ListArray>, String> {
        match self {
            ExpressionValue::List(list) => Ok(list),
            _ => Err("Expected list result".to_string()),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            ExpressionValue::Unit => "Unit",
            ExpressionValue::String(_) => "String",
            ExpressionValue::Boolean(_) => "Boolean",
            ExpressionValue::List(_) => "List",
            ExpressionValue::Option(_) => "Option",
        }
    }

    pub fn value_string(&self) -> String {
        match self {
            ExpressionValue::Unit => "()".to_string(),
            ExpressionValue::String(s) => s.clone(),
            ExpressionValue::Boolean(b) => b.to_string(),
            ExpressionValue::List(list) => format!("{:?}", list),
            ExpressionValue::Option(opt) => match opt {
                Some(value) => format!("Some({})", value.value_string()),
                None => "None".to_string(),
            },
        }
    }

    pub fn format_for_llm(&self) -> String {
        match self {
            ExpressionValue::String(s) => s.clone(),
            ExpressionValue::Unit => "()".to_string(),
            ExpressionValue::Boolean(b) => b.to_string(),
            ExpressionValue::List(list) => {
                if list.len() == 0 {
                    "[]".to_string()
                } else {
                    let values = list.value(0);
                    if let Some(string_array) =
                        values.as_any().downcast_ref::<arrow::array::StringArray>()
                    {
                        let items: Vec<String> = (0..string_array.len())
                            .map(|i| format!("\"{}\"", string_array.value(i)))
                            .collect();
                        format!("[{}]", items.join(", "))
                    } else {
                        "[]".to_string()
                    }
                }
            }
            ExpressionValue::Option(opt) => match opt {
                Some(inner) => format!("Some({})", inner.format_for_llm()),
                None => "None".to_string(),
            },
        }
    }
}

impl std::fmt::Display for ExpressionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value_string())
    }
}
