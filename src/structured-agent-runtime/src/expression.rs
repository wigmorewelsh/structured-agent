use arrow::array::{Array, ListArray};
use arrow::datatypes::{DataType, Field, Fields};
use std::sync::Arc;

use crate::runtime_value::{
    BooleanValue, IntValue, ListValue, MetadataValue, OptionValue, RuntimeValue, StringValue,
    StructValue, UnitValue, arrow_col_to_expression,
};
use crate::symbols::{DefinitionPath, MetaData, References, SymbolQuery, TypeDefinitionKind};
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
    Module {
        path: DefinitionPath,
        params: Vec<ExpressionValue>,
    },
    Dynamic(Arc<dyn RuntimeValue>),
}

impl PartialEq for ExpressionValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                ExpressionValue::Module {
                    path: a,
                    params: ap,
                },
                ExpressionValue::Module {
                    path: b,
                    params: bp,
                },
            ) => a == b && ap == bp,
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
        Self::Dynamic(Arc::new(StringValue(s.into())))
    }

    pub fn boolean(b: bool) -> Self {
        Self::Dynamic(Arc::new(BooleanValue(b)))
    }

    pub fn integer(n: i64) -> Self {
        Self::Dynamic(Arc::new(IntValue(n)))
    }

    pub fn list(arr: Arc<ListArray>) -> Self {
        Self::Dynamic(Arc::new(ListValue::new(arr)))
    }

    pub fn module(path: DefinitionPath) -> Self {
        Self::Module {
            path,
            params: vec![],
        }
    }

    pub fn from_elements(elements: Vec<ExpressionValue>) -> Result<Self, String> {
        Ok(Self::Dynamic(Arc::new(ListValue::from_elements(elements)?)))
    }

    pub fn struct_value(fields: Vec<(&str, ExpressionValue)>) -> Self {
        Self::Dynamic(Arc::new(StructValue::from_fields(fields)))
    }

    pub fn get_struct_field(&self, field: &str) -> Result<ExpressionValue, String> {
        match self {
            ExpressionValue::Dynamic(v) => {
                let sv = v
                    .as_any()
                    .downcast_ref::<StructValue>()
                    .ok_or_else(|| format!("Expected struct value, got {}", self.type_name()))?;
                sv.get_field(field)
            }
            _ => Err(format!("Expected struct value, got {}", self.type_name())),
        }
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
        Self::Dynamic(Arc::new(OptionValue::some(inner.to_arrow())))
    }

    pub fn metadata(name: impl Into<String>, documentation: Option<String>) -> Self {
        Self::Dynamic(Arc::new(MetadataValue {
            name: name.into(),
            documentation,
        }))
    }

    pub fn to_arrow(&self) -> Arc<dyn Array> {
        match self {
            ExpressionValue::Module { .. } => panic!("expected value, got Module"),
            ExpressionValue::Dynamic(v) => v.to_arrow(),
        }
    }

    pub fn as_string(&self) -> Result<String, String> {
        match self {
            ExpressionValue::Dynamic(v) => v
                .as_any()
                .downcast_ref::<StringValue>()
                .map(|sv| sv.0.clone())
                .ok_or_else(|| format!("expected String, got {}", self.type_name())),
            _ => Err(format!("expected String, got {}", self.type_name())),
        }
    }

    pub fn as_boolean(&self) -> Result<bool, String> {
        match self {
            ExpressionValue::Dynamic(v) => v
                .as_any()
                .downcast_ref::<BooleanValue>()
                .map(|bv| bv.0)
                .ok_or_else(|| format!("expected Boolean, got {}", self.type_name())),
            _ => Err(format!("expected Boolean, got {}", self.type_name())),
        }
    }

    pub fn as_integer(&self) -> Result<i64, String> {
        match self {
            ExpressionValue::Dynamic(v) => v
                .as_any()
                .downcast_ref::<IntValue>()
                .map(|iv| iv.0)
                .ok_or_else(|| format!("expected Int, got {}", self.type_name())),
            _ => Err(format!("expected Int, got {}", self.type_name())),
        }
    }

    pub fn as_list(&self) -> Result<&ListArray, String> {
        match self {
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
            .map(|i| arrow_col_to_expression(values.slice(i, 1)))
            .collect())
    }

    pub fn as_option(&self) -> Result<Option<ExpressionValue>, String> {
        match self {
            ExpressionValue::Dynamic(v) => {
                let opt = v
                    .as_any()
                    .downcast_ref::<OptionValue>()
                    .ok_or_else(|| "Expected option (union) type".to_string())?;
                Ok(opt.inner())
            }
            _ => Err("Expected option (union) type".to_string()),
        }
    }

    pub fn is_option(&self) -> bool {
        match self {
            ExpressionValue::Dynamic(v) => v.as_any().downcast_ref::<OptionValue>().is_some(),
            _ => false,
        }
    }

    pub fn as_module(&self) -> Result<&DefinitionPath, String> {
        match self {
            ExpressionValue::Module { path, .. } => Ok(path),
            _ => Err(format!("expected Module, got {}", self.type_name())),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            ExpressionValue::Module { .. } => "Module",
            ExpressionValue::Dynamic(v) => v.type_name(),
        }
    }

    pub fn as_metadata(&self) -> Result<(String, Option<String>), String> {
        match self {
            ExpressionValue::Dynamic(v) => {
                let mv = v
                    .as_any()
                    .downcast_ref::<MetadataValue>()
                    .ok_or_else(|| "Expected metadata struct".to_string())?;
                Ok((
                    mv.name().to_string(),
                    mv.documentation().map(str::to_string),
                ))
            }
            _ => Err("Expected metadata struct".to_string()),
        }
    }

    pub fn value_string(&self) -> String {
        match self {
            ExpressionValue::Module { path, .. } => format!("Module({})", path),
            ExpressionValue::Dynamic(v) => v.format_for_llm(),
        }
    }

    pub fn format_for_llm(&self) -> String {
        match self {
            ExpressionValue::Module { path, .. } => format!("Module({})", path),
            ExpressionValue::Dynamic(v) => v.format_for_llm(),
        }
    }

    pub fn downcast_clone<T: Clone + RuntimeValue + 'static>(&self) -> Result<T, String> {
        match self {
            ExpressionValue::Dynamic(v) => v.as_any().downcast_ref::<T>().cloned(),
            _ => None,
        }
        .ok_or_else(|| {
            format!(
                "expected {}, got {}",
                std::any::type_name::<T>(),
                self.type_name()
            )
        })
    }
}

pub fn type_to_arrow_datatype<R>(ty: &Type, metadata: &MetaData<R>) -> DataType
where
    R: References<TypeAnnotation = DefinitionPath>,
{
    match ty {
        Type::Parameterized(type_name, args)
            if type_name.last_name() == "List" && args.len() == 1 =>
        {
            let inner_dt = type_to_arrow_datatype(&args[0], metadata);
            DataType::List(Arc::new(Field::new("item", inner_dt, true)))
        }
        Type::Named(type_name) => type_name_to_arrow_datatype(type_name, &[], metadata),
        Type::Parameterized(type_name, args) => {
            let subst: Vec<(String, &Type)> = metadata
                .type_def(type_name)
                .and_then(|td| {
                    if let TypeDefinitionKind::Struct {
                        generic_parameters, ..
                    } = &td.kind
                    {
                        Some(
                            generic_parameters
                                .iter()
                                .map(|gp| gp.name.clone())
                                .zip(args.iter())
                                .collect(),
                        )
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            type_name_to_arrow_datatype(type_name, &subst, metadata)
        }
        Type::Generic(_) => DataType::Null,
    }
}

fn type_name_to_arrow_datatype<R>(
    type_name: &DefinitionPath,
    subst: &[(String, &Type)],
    metadata: &MetaData<R>,
) -> DataType
where
    R: References<TypeAnnotation = DefinitionPath>,
{
    if let Some((_, ty)) = subst.iter().find(|(k, _)| k == &type_name.last_name()) {
        return type_to_arrow_datatype(ty, metadata);
    }
    match type_name.last_name() {
        "Int" => DataType::Int64,
        "String" => DataType::Utf8,
        "Boolean" => DataType::Boolean,
        "Unit" => DataType::Null,
        _ => match metadata.type_def(type_name) {
            Some(td) => match &td.kind {
                TypeDefinitionKind::Struct { fields, .. } => {
                    let arrow_fields: Vec<Field> = fields
                        .iter()
                        .map(|f| {
                            Field::new(
                                &f.name,
                                type_name_to_arrow_datatype(&f.type_name, subst, metadata),
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
        DefinitionPath, FieldDefinition, GenericParameterDefinition, MetaData, NoAst,
        TypeDefinition, TypeDefinitionKind,
    };
    use crate::types::Type;

    use super::ExpressionValue;

    #[derive(Debug, Clone)]
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
        type TypeAnnotation = DefinitionPath;
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

    fn test_type_name() -> DefinitionPath {
        DefinitionPath::for_type(
            DefinitionPath::for_module(nonempty::nonempty!["test".to_string()]),
            "Test",
        )
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
        let data = opt.to_arrow();
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
        let struct_type_name = DefinitionPath::for_type(
            DefinitionPath::for_module(nonempty::nonempty!["test".to_string()]),
            "MyGeneric",
        );
        let string_type_name = DefinitionPath::for_type(
            DefinitionPath::for_module(nonempty::nonempty!["prelude".to_string()]),
            "String",
        );
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
        let ty = Type::Parameterized(struct_type_name, vec![Type::string()]);
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
    fn parameterized_type_with_generic_field_substitutes_type_arg() {
        let wrapper_type_name = DefinitionPath::for_type(
            DefinitionPath::for_module(nonempty::nonempty!["test".to_string()]),
            "Wrapper",
        );
        let t_type_name = DefinitionPath::for_type(
            DefinitionPath::for_module(nonempty::nonempty!["test".to_string()]),
            "T",
        );
        let mut metadata = MetaData::<TestRefs>::default();
        metadata.register_type(
            wrapper_type_name.clone(),
            Arc::new(TypeDefinition {
                name: wrapper_type_name.clone(),
                kind: TypeDefinitionKind::Struct {
                    fields: vec![FieldDefinition {
                        name: "value".to_string(),
                        type_name: t_type_name,
                    }],
                    generic_parameters: vec![GenericParameterDefinition {
                        name: "T".to_string(),
                        constraints: vec![],
                    }],
                },
                source_ref: NoSource,
                ast_ref: NoAst,
            }),
        );
        let ty = Type::Parameterized(wrapper_type_name, vec![Type::string()]);
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
            super::type_to_arrow_datatype(&Type::string(), &metadata),
            arrow::datatypes::DataType::Utf8
        );
        assert_eq!(
            super::type_to_arrow_datatype(&Type::boolean(), &metadata),
            arrow::datatypes::DataType::Boolean
        );
        assert_eq!(
            super::type_to_arrow_datatype(&Type::int(), &metadata),
            arrow::datatypes::DataType::Int64
        );
    }

    #[test]
    fn downcast_clone_integer_succeeds() {
        use crate::runtime_value::IntValue;
        let val = ExpressionValue::integer(42);
        let result = val.downcast_clone::<IntValue>();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, 42);
    }

    #[test]
    fn downcast_clone_wrong_type_fails() {
        use crate::runtime_value::IntValue;
        let val = ExpressionValue::string("x");
        let result = val.downcast_clone::<IntValue>();
        assert!(result.is_err());
    }
}
