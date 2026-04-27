use structured_agent_il::{
    Module, NativeFunctionDef, NativeImplDecl, NativeTraitDecl, NativeTraitFnDecl,
};
use structured_agent_runtime::{Parameter, Type};

mod int_impl {
    use structured_agent_macros::sa_fn;
    #[sa_fn]
    fn to_string(value: i64) -> String {
        value.to_string()
    }
}

mod string_impl {
    use structured_agent_macros::sa_fn;
    #[sa_fn]
    fn to_string(value: String) -> String {
        value
    }
}

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
                trait_name: Some("ToString".to_string()),
                functions: vec![int_impl::to_string_native_def()],
            },
            NativeImplDecl {
                type_name: "String".to_string(),
                trait_name: Some("ToString".to_string()),
                functions: vec![string_impl::to_string_native_def()],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{int_impl, string_impl};
    use structured_agent_il::Instruction;
    use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFnPtr};

    fn get_fn_ptr(def: &structured_agent_il::NativeFunctionDef) -> NativeFnPtr {
        if let Instruction::CallNative { f, .. } = &def.body[0] {
            f.clone()
        } else {
            panic!("expected CallNative instruction");
        }
    }

    #[tokio::test]
    async fn test_int_to_string() {
        let def = int_impl::to_string_native_def();
        let f = get_fn_ptr(&def);
        let result = f
            .call(vec![ExpressionValue::integer(42)], AgentHandle::detached())
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "42");
    }

    #[tokio::test]
    async fn test_string_to_string() {
        let def = string_impl::to_string_native_def();
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
