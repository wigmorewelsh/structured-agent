use arrow::array::ListBuilder;
use arrow::array::{
    Array, BooleanArray, BooleanBuilder, Int64Array, Int64Builder, ListArray, NullArray,
    StringArray, StringBuilder, StructArray, StructBuilder, UnionArray,
};
use arrow::buffer::OffsetBuffer;
use arrow::buffer::ScalarBuffer;
use arrow::compute::concat;
use arrow::datatypes::{DataType, Field, FieldRef, Fields, UnionFields};
use std::collections::HashMap;
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

    pub fn integer(n: i64) -> Self {
        Self {
            data: Arc::new(Int64Array::from(vec![n])),
        }
    }

    pub fn list(arr: Arc<ListArray>) -> Self {
        Self { data: arr }
    }

    pub fn from_elements(elements: Vec<ExpressionValue>) -> Result<Self, String> {
        if elements.is_empty() {
            let mut builder: ListBuilder<Box<dyn arrow::array::ArrayBuilder>> =
                ListBuilder::new(Box::new(StringBuilder::new()));
            return Ok(Self::list(Arc::new(builder.finish())));
        }

        match elements[0].type_name() {
            "String" => {
                let mut builder = ListBuilder::new(StringBuilder::new());
                for elem in &elements {
                    builder.values().append_value(elem.as_string()?);
                }
                builder.append(true);
                Ok(Self::list(Arc::new(builder.finish())))
            }
            "Int" => {
                let mut builder = ListBuilder::new(Int64Builder::new());
                for elem in &elements {
                    builder.values().append_value(elem.as_integer()?);
                }
                builder.append(true);
                Ok(Self::list(Arc::new(builder.finish())))
            }
            "Boolean" => {
                let mut builder = ListBuilder::new(BooleanBuilder::new());
                for elem in &elements {
                    builder.values().append_value(elem.as_boolean()?);
                }
                builder.append(true);
                Ok(Self::list(Arc::new(builder.finish())))
            }
            "Struct" => Self::list_from_structs(elements),
            "Option" => Self::list_from_options(elements),
            other => Err(format!("Unsupported list element type: {}", other)),
        }
    }

    fn list_from_structs(elements: Vec<ExpressionValue>) -> Result<Self, String> {
        let first = elements[0]
            .data
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or("Expected StructArray")?;

        let fields: Fields = first.fields().clone();
        let mut struct_builder = StructBuilder::from_fields(fields.clone(), elements.len());

        for elem in &elements {
            let sa = elem
                .data
                .as_any()
                .downcast_ref::<StructArray>()
                .ok_or("Expected StructArray in list")?;

            for (i, field) in fields.iter().enumerate() {
                let col = sa.column(i);
                match field.data_type() {
                    DataType::Utf8 => {
                        let b = struct_builder
                            .field_builder::<StringBuilder>(i)
                            .ok_or("Expected StringBuilder")?;
                        let val = col
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .ok_or("Expected StringArray")?;
                        b.append_value(val.value(0));
                    }
                    DataType::Int64 => {
                        let b = struct_builder
                            .field_builder::<Int64Builder>(i)
                            .ok_or("Expected Int64Builder")?;
                        let val = col
                            .as_any()
                            .downcast_ref::<Int64Array>()
                            .ok_or("Expected Int64Array")?;
                        b.append_value(val.value(0));
                    }
                    DataType::Boolean => {
                        let b = struct_builder
                            .field_builder::<BooleanBuilder>(i)
                            .ok_or("Expected BooleanBuilder")?;
                        let val = col
                            .as_any()
                            .downcast_ref::<BooleanArray>()
                            .ok_or("Expected BooleanArray")?;
                        b.append_value(val.value(0));
                    }
                    other => {
                        return Err(format!(
                            "Unsupported struct field type in list: {:?}",
                            other
                        ));
                    }
                }
            }
            struct_builder.append(true);
        }

        let child: Arc<dyn Array> = Arc::new(struct_builder.finish());
        let field = Arc::new(Field::new_struct("item", fields, true)) as FieldRef;
        let offsets = OffsetBuffer::new(vec![0i32, child.len() as i32].into());
        let list_array =
            ListArray::try_new(field, offsets, child, None).map_err(|e| e.to_string())?;
        Ok(Self::list(Arc::new(list_array)))
    }

    fn list_from_options(elements: Vec<ExpressionValue>) -> Result<Self, String> {
        let inner_type = elements
            .iter()
            .find_map(|e| {
                let ua = e.data.as_any().downcast_ref::<UnionArray>()?;
                if ua.type_id(0) == 1 {
                    Some(ua.value(0).data_type().clone())
                } else {
                    None
                }
            })
            .unwrap_or(DataType::Null);

        let normalised: Vec<Arc<dyn Array>> = elements
            .iter()
            .map(|e| {
                let ua = e
                    .data
                    .as_any()
                    .downcast_ref::<UnionArray>()
                    .ok_or("Expected UnionArray")?;

                if ua.type_id(0) == 0 {
                    let union_fields = UnionFields::try_new(
                        [0_i8, 1_i8],
                        [
                            Field::new("none", DataType::Null, true),
                            Field::new("some", inner_type.clone(), false),
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                    let type_ids: ScalarBuffer<i8> = [0_i8].into_iter().collect();
                    let offsets: ScalarBuffer<i32> = [0_i32].into_iter().collect();
                    let none_child = Arc::new(NullArray::new(1)) as Arc<dyn Array>;
                    let some_child = arrow::array::new_empty_array(&inner_type);
                    let ua = UnionArray::try_new(
                        union_fields,
                        type_ids,
                        Some(offsets),
                        vec![none_child, some_child],
                    )
                    .map_err(|e| e.to_string())?;
                    Ok(Arc::new(ua) as Arc<dyn Array>)
                } else {
                    Ok(e.data.clone())
                }
            })
            .collect::<Result<_, String>>()?;

        let refs: Vec<&dyn Array> = normalised.iter().map(|a| a.as_ref()).collect();
        let child = concat(&refs).map_err(|e| e.to_string())?;
        let child_type = child.data_type().clone();
        let field = Arc::new(Field::new("item", child_type, true)) as FieldRef;
        let offsets = OffsetBuffer::new(vec![0i32, child.len() as i32].into());
        let list_array =
            ListArray::try_new(field, offsets, child, None).map_err(|e| e.to_string())?;
        Ok(Self::list(Arc::new(list_array)))
    }

    pub fn from_array(data: Arc<dyn Array>) -> Self {
        Self { data }
    }

    pub fn struct_value(fields: Vec<(&str, ExpressionValue)>) -> Self {
        let arrow_fields: Vec<Field> = fields
            .iter()
            .map(|(field_name, val)| Field::new(*field_name, val.data.data_type().clone(), true))
            .collect();
        let arrays: Vec<Arc<dyn Array>> = fields.iter().map(|(_, val)| val.data.clone()).collect();
        let struct_array = StructArray::new(Fields::from(arrow_fields), arrays, None);
        Self {
            data: Arc::new(struct_array),
        }
    }

    pub fn get_struct_field(&self, field: &str) -> Result<ExpressionValue, String> {
        let struct_array = self
            .data
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| format!("Expected struct value, got {}", self.type_name()))?;
        let (index, _) = struct_array
            .fields()
            .iter()
            .enumerate()
            .find(|(_, f)| f.name() == field)
            .ok_or_else(|| format!("No field '{}' in struct", field))?;
        let col = struct_array.column(index);
        Ok(ExpressionValue { data: col.clone() })
    }

    pub fn option_none() -> Self {
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
            data: Arc::new(union_array),
        }
    }

    pub fn option_some(inner: ExpressionValue) -> Self {
        let inner_type = inner.data.data_type().clone();
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
        let children: Vec<Arc<dyn Array>> = vec![Arc::new(NullArray::new(0)), inner.data];
        let union_array = UnionArray::try_new(union_fields, type_ids, Some(offsets), children)
            .expect("valid option_some union");
        Self {
            data: Arc::new(union_array),
        }
    }

    pub fn metadata(name: impl Into<String>, documentation: Option<String>) -> Self {
        let name_str = name.into();
        let marker: HashMap<String, String> = [("kind".to_string(), "metadata".to_string())].into();
        let fields = Fields::from(vec![
            Field::new("name", DataType::Utf8, false).with_metadata(marker),
            Field::new("documentation", DataType::Utf8, true),
        ]);

        let name_array = Arc::new(StringArray::from(vec![name_str]));
        let doc_array = Arc::new(StringArray::from(vec![documentation]));

        let struct_array = StructArray::new(
            fields,
            vec![name_array as Arc<dyn Array>, doc_array as Arc<dyn Array>],
            None,
        );

        Self {
            data: Arc::new(struct_array),
        }
    }

    fn downcast_scalar<T: Array + 'static>(&self) -> Result<&T, String> {
        self.data
            .as_any()
            .downcast_ref::<T>()
            .filter(|arr| arr.len() == 1)
            .ok_or_else(|| format!("Expected scalar {}", std::any::type_name::<T>()))
    }

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

    pub fn as_integer(&self) -> Result<i64, String> {
        self.downcast_scalar::<Int64Array>().map(|arr| arr.value(0))
    }

    pub fn as_list(&self) -> Result<&ListArray, String> {
        self.data
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| "Expected list".to_string())
    }

    pub fn as_option(&self) -> Result<Option<ExpressionValue>, String> {
        let union_array = self
            .data
            .as_any()
            .downcast_ref::<UnionArray>()
            .ok_or_else(|| "Expected option (union) type".to_string())?;

        if union_array.len() != 1 {
            return Err("Expected scalar option value".to_string());
        }

        let type_id = union_array.type_id(0);
        match type_id {
            0 => Ok(None),
            1 => {
                let inner = union_array.value(0);
                Ok(Some(ExpressionValue { data: inner }))
            }
            _ => Err(format!("Unexpected union type_id: {}", type_id)),
        }
    }

    pub fn is_option(&self) -> bool {
        matches!(self.data.data_type(), DataType::Union(_, _))
    }

    pub fn type_name(&self) -> &str {
        match self.data.data_type() {
            DataType::Null => "Unit",
            DataType::Utf8 => "String",
            DataType::Boolean => "Boolean",
            DataType::Int64 => "Int",
            DataType::List(_) => "List",
            DataType::Struct(_) => {
                if let Some(struct_array) = self.data.as_any().downcast_ref::<StructArray>() {
                    if Self::is_metadata_struct(struct_array) {
                        return "Metadata";
                    }
                }
                "Struct"
            }
            DataType::Union(_, _) => "Option",
            _ => "Unknown",
        }
    }

    pub fn as_metadata(&self) -> Result<(String, Option<String>), String> {
        if let Some(struct_array) = self.data.as_any().downcast_ref::<StructArray>() {
            if !Self::is_metadata_struct(struct_array) {
                return Err("Not a metadata struct".to_string());
            }
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
        } else if let Ok(n) = self.as_integer() {
            n.to_string()
        } else if let Ok(list) = self.as_list() {
            format!("{:?}", list)
        } else if let Ok((name, documentation)) = self.as_metadata() {
            if let Some(doc) = documentation {
                format!("Metadata({}, \"{}\")", name, doc)
            } else {
                format!("Metadata({})", name)
            }
        } else if let Ok(opt) = self.as_option() {
            match opt {
                None => "None".to_string(),
                Some(inner) => format!("Some({})", inner.value_string()),
            }
        } else {
            format!("{:?}", self.data)
        }
    }

    fn is_metadata_struct(struct_array: &StructArray) -> bool {
        struct_array
            .fields()
            .first()
            .map(|f| {
                f.metadata()
                    .get("kind")
                    .map(|v| v == "metadata")
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub fn format_for_llm(&self) -> String {
        if let Some(struct_array) = self.data.as_any().downcast_ref::<StructArray>() {
            if !Self::is_metadata_struct(struct_array) {
                let mut obj = serde_json::Map::new();
                for (i, field) in struct_array.fields().iter().enumerate() {
                    let col = struct_array.column(i);
                    let val = ExpressionValue { data: col.clone() };
                    let json_val = if let Ok(s) = val.as_string() {
                        serde_json::Value::String(s.to_string())
                    } else if let Ok(b) = val.as_boolean() {
                        serde_json::Value::Bool(b)
                    } else if let Ok(n) = val.as_integer() {
                        serde_json::Value::Number(n.into())
                    } else {
                        serde_json::Value::String(val.format_for_llm())
                    };
                    obj.insert(field.name().clone(), json_val);
                }
                return serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string());
            }
        }
        if self.data.as_any().is::<NullArray>() {
            "()".to_string()
        } else if let Ok(s) = self.as_string() {
            s.to_string()
        } else if let Ok(b) = self.as_boolean() {
            b.to_string()
        } else if let Ok(n) = self.as_integer() {
            n.to_string()
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
                } else if let Some(int_array) = values.as_any().downcast_ref::<Int64Array>() {
                    let items: Vec<String> = (0..int_array.len())
                        .map(|i| int_array.value(i).to_string())
                        .collect();
                    format!("[{}]", items.join(", "))
                } else if let Some(bool_array) = values.as_any().downcast_ref::<BooleanArray>() {
                    let items: Vec<String> = (0..bool_array.len())
                        .map(|i| bool_array.value(i).to_string())
                        .collect();
                    format!("[{}]", items.join(", "))
                } else if let Some(struct_array) = values.as_any().downcast_ref::<StructArray>() {
                    let items: Vec<String> = (0..struct_array.len())
                        .map(|i| {
                            let mut obj = serde_json::Map::new();
                            for (j, field) in struct_array.fields().iter().enumerate() {
                                let col = struct_array.column(j);
                                let val = ExpressionValue {
                                    data: col.slice(i, 1),
                                };
                                let json_val = if let Ok(s) = val.as_string() {
                                    serde_json::Value::String(s.to_string())
                                } else if let Ok(b) = val.as_boolean() {
                                    serde_json::Value::Bool(b)
                                } else if let Ok(n) = val.as_integer() {
                                    serde_json::Value::Number(n.into())
                                } else {
                                    serde_json::Value::String(val.format_for_llm())
                                };
                                obj.insert(field.name().clone(), json_val);
                            }
                            serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string())
                        })
                        .collect();
                    format!("[{}]", items.join(", "))
                } else if let Some(union_array) = values.as_any().downcast_ref::<UnionArray>() {
                    let items: Vec<String> = (0..union_array.len())
                        .map(|i| {
                            let sliced = ExpressionValue {
                                data: Arc::new(union_array.slice(i, 1)) as Arc<dyn Array>,
                            };
                            match sliced.as_option() {
                                Ok(None) => "None".to_string(),
                                Ok(Some(inner)) => format!("Some({})", inner.format_for_llm()),
                                Err(_) => ExpressionValue {
                                    data: union_array.value(i),
                                }
                                .format_for_llm(),
                            }
                        })
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
        } else if let Ok(opt) = self.as_option() {
            match opt {
                None => "None".to_string(),
                Some(inner) => inner.format_for_llm(),
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
