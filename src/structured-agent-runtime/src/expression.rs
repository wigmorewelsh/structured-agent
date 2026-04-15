use arrow::array::{
    Array, BooleanArray, Int64Array, ListArray, NullArray, StringArray, StructArray, UnionArray,
};
use arrow::datatypes::{DataType, Field, Fields};
use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime_value::{ListValue, OptionValue, RuntimeValue, UnitValue};
use crate::symbols::{
    FunctionName, MetaData, References, SymbolQuery, TypeDefinitionKind, TypeName,
};
use crate::types::Type;

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
pub enum ExpressionValue {
    Arrow(Arc<dyn Array>),
    Module(FunctionName),
    Dynamic(Arc<dyn RuntimeValue>),
}

impl PartialEq for ExpressionValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExpressionValue::Arrow(a), ExpressionValue::Arrow(b)) => a.as_ref() == b.as_ref(),
            (ExpressionValue::Module(a), ExpressionValue::Module(b)) => a == b,
            (ExpressionValue::Dynamic(a), ExpressionValue::Dynamic(b)) => a.eq(b.as_any()),
            _ => false,
        }
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
        Self::Dynamic(Arc::new(UnitValue))
    }

    pub fn string(s: impl Into<String>) -> Self {
        Self::Arrow(Arc::new(StringArray::from(vec![s.into()])))
    }

    pub fn boolean(b: bool) -> Self {
        Self::Arrow(Arc::new(BooleanArray::from(vec![b])))
    }

    pub fn integer(n: i64) -> Self {
        Self::Arrow(Arc::new(Int64Array::from(vec![n])))
    }

    pub fn list(arr: Arc<ListArray>) -> Self {
        Self::Dynamic(Arc::new(ListValue::new(arr)))
    }

    pub fn module(name: FunctionName) -> Self {
        Self::Module(name)
    }

    pub fn from_elements(elements: Vec<ExpressionValue>) -> Result<Self, String> {
        Ok(Self::Dynamic(Arc::new(ListValue::from_elements(elements)?)))
    }

    pub fn from_array(data: Arc<dyn Array>) -> Self {
        Self::Arrow(data)
    }

    pub fn struct_value(fields: Vec<(&str, ExpressionValue)>) -> Self {
        let arrow_fields: Vec<Field> = fields
            .iter()
            .map(|(field_name, val)| {
                Field::new(*field_name, val.arrow_data().data_type().clone(), true)
            })
            .collect();
        let arrays: Vec<Arc<dyn Array>> = fields
            .iter()
            .map(|(_, val)| val.arrow_data().clone())
            .collect();
        let struct_array = StructArray::new(Fields::from(arrow_fields), arrays, None);
        Self::Arrow(Arc::new(struct_array))
    }

    pub fn get_struct_field(&self, field: &str) -> Result<ExpressionValue, String> {
        let data = self.arrow_data();
        let struct_array = data
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
        Ok(ExpressionValue::Arrow(col.clone()))
    }

    pub fn option_none() -> Self {
        Self::Dynamic(Arc::new(OptionValue::none()))
    }

    pub fn option_none_utf8() -> Self {
        Self::Dynamic(Arc::new(OptionValue::none_utf8()))
    }

    pub fn option_none_boolean() -> Self {
        Self::Dynamic(Arc::new(OptionValue::none_boolean()))
    }

    pub fn option_none_int64() -> Self {
        Self::Dynamic(Arc::new(OptionValue::none_int64()))
    }

    pub fn option_none_with_type(inner_type: DataType) -> Self {
        Self::Dynamic(Arc::new(OptionValue::none_with_type(inner_type)))
    }

    pub fn option_some(inner: ExpressionValue) -> Self {
        Self::Dynamic(Arc::new(OptionValue::some(inner.arrow_data())))
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

        Self::Arrow(Arc::new(struct_array))
    }

    pub(crate) fn arrow_data(&self) -> Arc<dyn Array> {
        match self {
            ExpressionValue::Arrow(data) => data.clone(),
            ExpressionValue::Module(_) => panic!("expected Arrow value, got Module"),
            ExpressionValue::Dynamic(v) => v.to_arrow(),
        }
    }

    pub fn as_string(&self) -> Result<String, String> {
        let data = self.arrow_data();
        data.as_any()
            .downcast_ref::<StringArray>()
            .filter(|arr| arr.len() == 1 && !arr.is_null(0))
            .map(|arr| arr.value(0).to_string())
            .ok_or_else(|| format!("expected String, got {}", self.type_name()))
    }

    pub fn as_boolean(&self) -> Result<bool, String> {
        let data = self.arrow_data();
        data.as_any()
            .downcast_ref::<BooleanArray>()
            .filter(|arr| arr.len() == 1 && !arr.is_null(0))
            .map(|arr| arr.value(0))
            .ok_or_else(|| format!("expected Boolean, got {}", self.type_name()))
    }

    pub fn as_integer(&self) -> Result<i64, String> {
        let data = self.arrow_data();
        data.as_any()
            .downcast_ref::<Int64Array>()
            .filter(|arr| arr.len() == 1)
            .map(|arr| arr.value(0))
            .ok_or_else(|| format!("expected Int, got {}", self.type_name()))
    }

    pub fn as_list(&self) -> Result<&ListArray, String> {
        match self {
            ExpressionValue::Arrow(data) => data
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| "Expected list".to_string()),
            ExpressionValue::Dynamic(v) => v
                .as_any()
                .downcast_ref::<ListValue>()
                .map(|lv| lv.list_array())
                .ok_or_else(|| format!("expected List, got {}", self.type_name())),
            _ => Err(format!("expected List, got {}", self.type_name())),
        }
    }

    pub fn as_list_elements(&self) -> Result<Vec<ExpressionValue>, String> {
        let list = self.as_list()?;
        if list.is_empty() {
            return Ok(vec![]);
        }
        let values = list.value(0);
        Ok((0..values.len())
            .map(|i| ExpressionValue::Arrow(values.slice(i, 1)))
            .collect())
    }

    pub fn as_option(&self) -> Result<Option<ExpressionValue>, String> {
        let data = self.arrow_data();
        let union_array = data
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
                Ok(Some(ExpressionValue::Arrow(inner)))
            }
            _ => Err(format!("Unexpected union type_id: {}", type_id)),
        }
    }

    pub fn is_option(&self) -> bool {
        match self {
            ExpressionValue::Arrow(data) => matches!(data.data_type(), DataType::Union(_, _)),
            ExpressionValue::Dynamic(v) => v.as_any().downcast_ref::<OptionValue>().is_some(),
            ExpressionValue::Module(_) => false,
        }
    }

    pub fn as_module(&self) -> Result<&FunctionName, String> {
        match self {
            ExpressionValue::Module(name) => Ok(name),
            _ => Err(format!("expected Module, got {}", self.type_name())),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            ExpressionValue::Module(_) => "Module",
            ExpressionValue::Dynamic(v) => v.type_name(),
            ExpressionValue::Arrow(data) => match data.data_type() {
                DataType::Null => "Unit",
                DataType::Utf8 => "String",
                DataType::Boolean => "Boolean",
                DataType::Int64 => "Int",
                DataType::List(_) => "List",
                DataType::Struct(_) => {
                    if let Some(struct_array) = data.as_any().downcast_ref::<StructArray>()
                        && Self::is_metadata_struct(struct_array)
                    {
                        return "Metadata";
                    }
                    "Struct"
                }
                DataType::Union(_, _) => "Option",
                _ => "Unknown",
            },
        }
    }

    pub fn as_metadata(&self) -> Result<(String, Option<String>), String> {
        let data = self.arrow_data();
        if let Some(struct_array) = data.as_any().downcast_ref::<StructArray>() {
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
        if let ExpressionValue::Module(name) = self {
            return format!("Module({})", name);
        }
        if let ExpressionValue::Dynamic(v) = self {
            return v.format_for_llm();
        }
        let data = self.arrow_data();
        if data.as_any().is::<NullArray>() {
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
            format!("{:?}", data)
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
        if let ExpressionValue::Module(name) = self {
            return format!("Module({})", name);
        }
        if let ExpressionValue::Dynamic(v) = self {
            return v.format_for_llm();
        }
        let data = self.arrow_data();
        if let Some(struct_array) = data.as_any().downcast_ref::<StructArray>()
            && !Self::is_metadata_struct(struct_array)
        {
            let mut obj = serde_json::Map::new();
            for (i, field) in struct_array.fields().iter().enumerate() {
                let col = struct_array.column(i);
                let val = ExpressionValue::Arrow(col.clone());
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
        if data.as_any().is::<NullArray>() {
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
                                let val = ExpressionValue::Arrow(col.slice(i, 1));
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
                            let sliced = ExpressionValue::Arrow(
                                Arc::new(union_array.slice(i, 1)) as Arc<dyn Array>
                            );
                            match sliced.as_option() {
                                Ok(None) => "None".to_string(),
                                Ok(Some(inner)) => format!("Some({})", inner.format_for_llm()),
                                Err(_) => {
                                    ExpressionValue::Arrow(union_array.value(i)).format_for_llm()
                                }
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

pub fn type_to_arrow_datatype<R>(ty: &Type, metadata: &MetaData<R>) -> DataType
where
    R: References<TypeAnnotation = TypeName>,
{
    match ty {
        Type::String => DataType::Utf8,
        Type::Boolean => DataType::Boolean,
        Type::Int => DataType::Int64,
        Type::Unit => DataType::Null,
        Type::Parameterized(type_name, args) if type_name.name == "List" && args.len() == 1 => {
            let inner_dt = type_to_arrow_datatype(&args[0], metadata);
            DataType::List(Arc::new(Field::new("item", inner_dt, true)))
        }
        Type::Struct(type_name) => type_name_to_arrow_datatype(type_name, metadata),
        Type::Parameterized(type_name, _) => type_name_to_arrow_datatype(type_name, metadata),
        Type::Generic(_) => DataType::Null,
    }
}

fn type_name_to_arrow_datatype<R>(type_name: &TypeName, metadata: &MetaData<R>) -> DataType
where
    R: References<TypeAnnotation = TypeName>,
{
    match type_name.name.as_str() {
        "Int" => DataType::Int64,
        "String" => DataType::Utf8,
        "Boolean" => DataType::Boolean,
        "Unit" | "()" => DataType::Null,
        _ => match metadata.type_def(type_name) {
            Some(td) => match &td.kind {
                TypeDefinitionKind::Struct { fields, .. } => {
                    let arrow_fields: Vec<Field> = fields
                        .iter()
                        .map(|f| {
                            Field::new(
                                &f.name,
                                type_name_to_arrow_datatype(&f.type_name, metadata),
                                true,
                            )
                        })
                        .collect();
                    DataType::Struct(Fields::from(arrow_fields))
                }
                _ => DataType::Null,
            },
            None => DataType::Null,
        },
    }
}

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{NullArray, UnionArray};
    use arrow::datatypes::{DataType, Field, Fields};

    use crate::runtime_value::{RuntimeValue, RuntimeValueFactory, UnitValue};
    use crate::symbols::{
        FieldDefinition, GenericParameterDefinition, MetaData, ModuleName, NoAst, TypeDefinition,
        TypeDefinitionKind, TypeName,
    };
    use crate::types::Type;

    use super::ExpressionValue;

    struct NoSource;
    impl crate::symbols::SourceRef for NoSource {}
    struct NoBody;
    impl crate::symbols::BodyRef for NoBody {}
    #[derive(Debug, Clone, Default)]
    struct NoWitness;
    impl crate::symbols::WitnessRef for NoWitness {}

    struct TestRefs;
    impl crate::symbols::References for TestRefs {
        type Source = NoSource;
        type Ast = NoAst;
        type Body = NoBody;
        type Witness = NoWitness;
        type TypeAnnotation = TypeName;
    }

    #[derive(Debug)]
    struct NullFactory;

    impl RuntimeValueFactory for NullFactory {
        fn type_name(&self) -> &str {
            "Null"
        }

        fn construct(&self, _args: Vec<ExpressionValue>) -> Arc<dyn RuntimeValue> {
            Arc::new(UnitValue)
        }
    }

    fn test_type_name() -> TypeName {
        TypeName {
            name: "Test".to_string(),
            module: ModuleName::new(nonempty::nonempty!["test".to_string()]),
        }
    }

    #[test]
    fn unit_equals_unit() {
        assert_eq!(ExpressionValue::unit(), ExpressionValue::unit());
    }

    #[test]
    fn unit_not_equal_to_string() {
        assert_ne!(ExpressionValue::unit(), ExpressionValue::string("hello"));
    }

    #[test]
    fn unit_type_name() {
        assert_eq!(ExpressionValue::unit().type_name(), "Unit");
    }

    #[test]
    fn unit_to_arrow_is_null_array() {
        let v = UnitValue;
        let arr = v.to_arrow();
        assert!(arr.as_any().downcast_ref::<NullArray>().is_some());
    }

    #[test]
    fn unit_eq_same_type() {
        let a = UnitValue;
        let b = UnitValue;
        assert!(a.eq(b.as_any()));
    }

    #[test]
    fn native_type_definition_kind() {
        let def = TypeDefinition::<TestRefs> {
            name: test_type_name(),
            kind: TypeDefinitionKind::Native {
                generic_parameters: vec![GenericParameterDefinition {
                    name: "T".to_string(),
                    constraints: vec![],
                }],
                factory: Arc::new(NullFactory),
            },
            source_ref: NoSource,
            ast_ref: NoAst,
        };
        assert!(matches!(def.kind, TypeDefinitionKind::Native { .. }));
    }

    #[test]
    fn unit_value_string_is_unit_literal() {
        assert_eq!(ExpressionValue::unit().value_string(), "()");
    }

    #[test]
    fn unit_format_for_llm_is_unit_literal() {
        assert_eq!(ExpressionValue::unit().format_for_llm(), "()");
    }

    #[test]
    fn unit_as_string_is_err() {
        assert!(ExpressionValue::unit().as_string().is_err());
    }

    #[test]
    fn unit_as_boolean_is_err() {
        assert!(ExpressionValue::unit().as_boolean().is_err());
    }

    #[test]
    fn unit_as_integer_is_err() {
        assert!(ExpressionValue::unit().as_integer().is_err());
    }

    #[test]
    fn unit_as_list_is_err() {
        assert!(ExpressionValue::unit().as_list().is_err());
    }

    #[test]
    fn unit_as_option_is_err() {
        assert!(ExpressionValue::unit().as_option().is_err());
    }

    #[test]
    fn unit_as_metadata_is_err() {
        assert!(ExpressionValue::unit().as_metadata().is_err());
    }

    #[test]
    fn list_type_name() {
        let list = ExpressionValue::from_elements(vec![
            ExpressionValue::string("a"),
            ExpressionValue::string("b"),
        ])
        .unwrap();
        assert_eq!(list.type_name(), "List");
    }

    #[test]
    fn list_equals_list() {
        let a = ExpressionValue::from_elements(vec![ExpressionValue::integer(1)]).unwrap();
        let b = ExpressionValue::from_elements(vec![ExpressionValue::integer(1)]).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn list_not_equal_different_contents() {
        let a = ExpressionValue::from_elements(vec![ExpressionValue::integer(1)]).unwrap();
        let b = ExpressionValue::from_elements(vec![ExpressionValue::integer(2)]).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn list_as_list_returns_list_array() {
        let list = ExpressionValue::from_elements(vec![ExpressionValue::string("x")]).unwrap();
        assert!(list.as_list().is_ok());
    }

    #[test]
    fn list_to_arrow_is_list_array() {
        use crate::runtime_value::ListValue;
        let lv = ListValue::new(Arc::new(arrow::array::ListArray::from_iter_primitive::<
            arrow::datatypes::Int64Type,
            _,
            _,
        >(vec![Some(vec![Some(1i64)])])));
        let arr = lv.to_arrow();
        assert!(
            arr.as_any()
                .downcast_ref::<arrow::array::ListArray>()
                .is_some()
        );
    }

    #[test]
    fn option_none_type_name() {
        assert_eq!(ExpressionValue::option_none().type_name(), "Option");
    }

    #[test]
    fn option_some_type_name() {
        assert_eq!(
            ExpressionValue::option_some(ExpressionValue::string("hi")).type_name(),
            "Option"
        );
    }

    #[test]
    fn option_none_equals_option_none() {
        assert_eq!(
            ExpressionValue::option_none(),
            ExpressionValue::option_none()
        );
    }

    #[test]
    fn option_some_equals_option_some() {
        let a = ExpressionValue::option_some(ExpressionValue::string("hi"));
        let b = ExpressionValue::option_some(ExpressionValue::string("hi"));
        assert_eq!(a, b);
    }

    #[test]
    fn option_some_not_equal_option_none() {
        let a = ExpressionValue::option_some(ExpressionValue::string("hi"));
        let b = ExpressionValue::option_none();
        assert_ne!(a, b);
    }

    #[test]
    fn is_option_true_for_dynamic_option() {
        assert!(ExpressionValue::option_none().is_option());
        assert!(ExpressionValue::option_some(ExpressionValue::boolean(true)).is_option());
    }

    #[test]
    fn is_option_false_for_list() {
        let list = ExpressionValue::from_elements(vec![ExpressionValue::string("a")]).unwrap();
        assert!(!list.is_option());
    }

    #[test]
    fn as_option_none_returns_none() {
        let opt = ExpressionValue::option_none().as_option().unwrap();
        assert!(opt.is_none());
    }

    #[test]
    fn as_option_some_returns_some() {
        let opt = ExpressionValue::option_some(ExpressionValue::string("hi"))
            .as_option()
            .unwrap();
        assert!(opt.is_some());
    }

    #[test]
    fn option_none_utf8_has_utf8_some_schema() {
        let opt = ExpressionValue::option_none_utf8();
        let data = opt.arrow_data();
        let ua = data.as_any().downcast_ref::<UnionArray>().unwrap();
        let (_, some_field): (i8, &arrow::datatypes::FieldRef) = ua.fields().iter().nth(1).unwrap();
        assert_eq!(some_field.data_type(), &arrow::datatypes::DataType::Utf8);
    }

    #[test]
    fn option_list_with_typed_none_uses_concat_path() {
        let some = ExpressionValue::option_some(ExpressionValue::string("hello"));
        let none = ExpressionValue::option_none_utf8();
        let list = ExpressionValue::from_elements(vec![some, none]).unwrap();
        assert_eq!(list.type_name(), "List");
        let arr = list.as_list().unwrap();
        let values = arr.value(0);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn parameterized_type_to_arrow_datatype_delegates_to_struct() {
        let struct_type_name = TypeName {
            name: "MyGeneric".to_string(),
            module: ModuleName::new(nonempty::nonempty!["test".to_string()]),
        };
        let string_type_name = TypeName {
            name: "String".to_string(),
            module: ModuleName::new(nonempty::nonempty!["prelude".to_string()]),
        };
        let mut metadata = MetaData::<TestRefs>::default();
        metadata.register_type(
            struct_type_name.clone(),
            Arc::new(TypeDefinition {
                name: struct_type_name.clone(),
                kind: TypeDefinitionKind::Struct {
                    fields: vec![FieldDefinition {
                        name: "value".to_string(),
                        type_name: string_type_name,
                    }],
                    generic_parameters: vec![],
                },
                source_ref: NoSource,
                ast_ref: NoAst,
            }),
        );
        let ty = Type::Parameterized(struct_type_name, vec![Type::String]);
        assert_eq!(
            super::type_to_arrow_datatype(&ty, &metadata),
            DataType::Struct(Fields::from(vec![Field::new(
                "value",
                DataType::Utf8,
                true
            )]))
        );
    }

    #[test]
    fn rt_type_to_arrow_datatype_maps_primitives() {
        let metadata = MetaData::<TestRefs>::default();
        assert_eq!(
            super::type_to_arrow_datatype(&Type::String, &metadata),
            arrow::datatypes::DataType::Utf8
        );
        assert_eq!(
            super::type_to_arrow_datatype(&Type::Boolean, &metadata),
            arrow::datatypes::DataType::Boolean
        );
        assert_eq!(
            super::type_to_arrow_datatype(&Type::Int, &metadata),
            arrow::datatypes::DataType::Int64
        );
    }
}
