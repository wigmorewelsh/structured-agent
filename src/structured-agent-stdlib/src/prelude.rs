use std::sync::Arc;
use structured_agent_il::{
    Instruction, Module, NativeFunctionDef, NativeImplDecl, NativeTraitDecl, NativeTraitFnDecl,
    Slot,
};
use structured_agent_runtime::{
    ExpressionValue, NativeFnPtr, Parameter, Type,
    runtime_value::{
        AudioValueFactory, BooleanValueFactory, ImageValueFactory, IntValueFactory,
        LinkValueFactory, ListValueFactory, RuntimeValueFactory, StringValueFactory,
        UnitValueFactory,
    },
};

pub struct PreludeModule;

impl Module for PreludeModule {
    fn name(&self) -> &str {
        "prelude"
    }

    fn native_functions(&self) -> Vec<NativeFunctionDef> {
        vec![]
    }

    fn native_traits(&self) -> Vec<NativeTraitDecl> {
        vec![NativeTraitDecl {
            name: "ToString".to_string(),
            functions: vec![NativeTraitFnDecl {
                name: "to_string".to_string(),
                parameters: vec![Parameter::new("self".to_string(), Type::generic("Self"))],
                return_type: Type::string(),
            }],
        }]
    }

    fn native_impls(&self) -> Vec<NativeImplDecl> {
        vec![
            NativeImplDecl {
                type_name: "Int".to_string(),
                type_params: vec![],
                trait_name: Some("ToString".to_string()),
                functions: vec![int_to_string_def()],
            },
            NativeImplDecl {
                type_name: "String".to_string(),
                type_params: vec![],
                trait_name: Some("ToString".to_string()),
                functions: vec![string_to_string_def()],
            },
        ]
    }

    fn native_types(&self) -> Vec<Arc<dyn RuntimeValueFactory>> {
        vec![
            Arc::new(UnitValueFactory),
            Arc::new(BooleanValueFactory),
            Arc::new(StringValueFactory),
            Arc::new(IntValueFactory),
            Arc::new(ImageValueFactory),
            Arc::new(AudioValueFactory),
            Arc::new(LinkValueFactory),
            Arc::new(ListValueFactory),
        ]
    }
}

fn int_to_string_def() -> NativeFunctionDef {
    NativeFunctionDef::new(
        "to_string".to_string(),
        vec![Parameter::new("self".to_string(), Type::int())],
        Type::string(),
        vec![],
        None,
        vec![
            Instruction::CallNative {
                f: NativeFnPtr::new(|args, _agent| {
                    Box::pin(async move {
                        let v = args[0]
                            .as_integer()
                            .map_err(|_| "expected Int".to_string())?;
                        Ok(ExpressionValue::string(v.to_string()))
                    })
                }),
                params: vec![Slot(1)],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ],
    )
}

fn string_to_string_def() -> NativeFunctionDef {
    NativeFunctionDef::new(
        "to_string".to_string(),
        vec![Parameter::new("self".to_string(), Type::string())],
        Type::string(),
        vec![],
        None,
        vec![
            Instruction::CallNative {
                f: NativeFnPtr::new(|args, _agent| {
                    Box::pin(async move {
                        let s = args[0]
                            .as_string()
                            .map_err(|_| "expected String".to_string())?;
                        Ok(ExpressionValue::string(s))
                    })
                }),
                params: vec![Slot(1)],
                dest: Slot(0),
            },
            Instruction::Ret { var: Slot(0) },
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use structured_agent_runtime::AgentHandle;

    fn get_fn_ptr(def: &NativeFunctionDef) -> NativeFnPtr {
        if let Instruction::CallNative { f, .. } = &def.body[0] {
            f.clone()
        } else {
            panic!("expected CallNative instruction");
        }
    }

    #[tokio::test]
    async fn test_int_to_string() {
        let def = int_to_string_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(vec![ExpressionValue::integer(42)], AgentHandle::detached())
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "42");
    }

    #[tokio::test]
    async fn test_string_to_string() {
        let def = string_to_string_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(
                vec![ExpressionValue::string("hello")],
                AgentHandle::detached(),
            )
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "hello");
    }
}
