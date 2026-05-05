use structured_agent_il::{
    Instruction, Module, NativeFunctionDef, NativeImplDecl, NativeTraitDecl, NativeTraitFnDecl,
    Slot,
};
use structured_agent_runtime::{
    DefinitionPath, ExpressionValue, ListValue, NativeFnPtr, Parameter, Type,
};

pub struct IteratorModule;

fn module_path() -> DefinitionPath {
    DefinitionPath::root().with_module("iterator".to_string())
}

impl Module for IteratorModule {
    fn name(&self) -> &str {
        "iterator"
    }

    fn native_functions(&self) -> Vec<NativeFunctionDef> {
        vec![]
    }

    fn native_traits(&self) -> Vec<NativeTraitDecl> {
        vec![NativeTraitDecl {
            name: "Iterator".to_string(),
            functions: vec![
                NativeTraitFnDecl {
                    name: "move_next".to_string(),
                    parameters: vec![Parameter::new("self".to_string(), Type::generic("Self"))],
                    return_type: Type::boolean(),
                },
                NativeTraitFnDecl {
                    name: "current".to_string(),
                    parameters: vec![Parameter::new("self".to_string(), Type::generic("Self"))],
                    return_type: Type::generic("T"),
                },
            ],
        }]
    }

    fn native_impls(&self) -> Vec<NativeImplDecl> {
        let mp = module_path();
        let return_type = Type::Parameterized(
            DefinitionPath::for_type(mp.clone(), "ListIterator"),
            vec![Type::generic("T")],
        );
        let iter_fn = NativeFunctionDef::new(
            "iter".to_string(),
            vec![Parameter::new(
                "self".to_string(),
                Type::list(Type::generic("T")),
            )],
            return_type,
            vec!["T".to_string()],
            None,
            vec![
                Instruction::CallNative {
                    f: NativeFnPtr::new(|args, _agent| {
                        Box::pin(async move {
                            let list_val = match &args[0] {
                                ExpressionValue::Dynamic(v) => {
                                    v.as_any().downcast_ref::<ListValue>()
                                }
                                _ => None,
                            }
                            .ok_or_else(|| "expected List".to_string())?;
                            let arc = list_val.list_arc();
                            Ok(ExpressionValue::list_iterator(arc))
                        })
                    }),
                    params: vec![Slot(2)],
                    dest: Slot(0),
                },
                Instruction::Ret { var: Slot(0) },
            ],
        );
        let move_next_def = NativeFunctionDef::new(
            "move_next".to_string(),
            vec![Parameter::new("self".to_string(), Type::generic("Self"))],
            Type::boolean(),
            vec![],
            None,
            vec![
                Instruction::CallNative {
                    f: NativeFnPtr::new(|args, _agent| {
                        Box::pin(async move {
                            let iter = args[0].as_list_iterator()?;
                            let result = iter.move_next();
                            Ok(ExpressionValue::boolean(result))
                        })
                    }),
                    params: vec![Slot(1)],
                    dest: Slot(0),
                },
                Instruction::Ret { var: Slot(0) },
            ],
        );
        let current_def = NativeFunctionDef::new(
            "current".to_string(),
            vec![Parameter::new("self".to_string(), Type::generic("Self"))],
            Type::generic("T"),
            vec![],
            None,
            vec![
                Instruction::CallNative {
                    f: NativeFnPtr::new(|args, _agent| {
                        Box::pin(async move {
                            let iter = args[0].as_list_iterator()?;
                            iter.current().ok_or_else(|| {
                                "current() called on iterator with no current element".to_string()
                            })
                        })
                    }),
                    params: vec![Slot(1)],
                    dest: Slot(0),
                },
                Instruction::Ret { var: Slot(0) },
            ],
        );
        vec![
            NativeImplDecl {
                type_name: "ListIterator".to_string(),
                type_params: vec!["T".to_string()],
                trait_name: Some("Iterator".to_string()),
                functions: vec![move_next_def, current_def],
            },
            NativeImplDecl {
                type_name: "List".to_string(),
                type_params: vec![],
                trait_name: None,
                functions: vec![iter_fn],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use structured_agent_il::{Instruction, Module};
    use structured_agent_runtime::{AgentHandle, ExpressionValue};

    fn get_fn_ptr(def: &NativeFunctionDef) -> NativeFnPtr {
        if let Instruction::CallNative { f, .. } = &def.body[0] {
            f.clone()
        } else {
            panic!("expected CallNative instruction");
        }
    }

    fn make_string_list(items: Vec<&str>) -> ExpressionValue {
        let values: Vec<ExpressionValue> =
            items.iter().map(|s| ExpressionValue::string(*s)).collect();
        ExpressionValue::from_elements(values).unwrap()
    }

    #[tokio::test]
    async fn iterator_module_name() {
        assert_eq!(IteratorModule.name(), "iterator");
    }

    #[tokio::test]
    async fn iterator_trait_decl_name() {
        let traits = IteratorModule.native_traits();
        assert_eq!(traits[0].name, "Iterator");
    }

    #[tokio::test]
    async fn iterator_trait_has_move_next_and_current() {
        let traits = IteratorModule.native_traits();
        let fns: Vec<&str> = traits[0]
            .functions
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert!(fns.contains(&"move_next"));
        assert!(fns.contains(&"current"));
    }

    #[tokio::test]
    async fn current_fn_decl_return_type_is_generic_t() {
        let traits = IteratorModule.native_traits();
        let current = traits[0]
            .functions
            .iter()
            .find(|f| f.name == "current")
            .unwrap();
        assert_eq!(current.return_type, Type::generic("T"));
    }

    #[tokio::test]
    async fn move_next_fn_decl_return_type_is_boolean() {
        let traits = IteratorModule.native_traits();
        let mn = traits[0]
            .functions
            .iter()
            .find(|f| f.name == "move_next")
            .unwrap();
        assert_eq!(mn.return_type, Type::boolean());
    }

    #[tokio::test]
    async fn iter_native_fn_on_list_returns_list_iterator() {
        let impls = IteratorModule.native_impls();
        let list_impl = impls.iter().find(|i| i.type_name == "List").unwrap();
        let iter_def = list_impl
            .functions
            .iter()
            .find(|f| f.name == "iter")
            .unwrap();
        let f = get_fn_ptr(iter_def);
        let list = make_string_list(vec!["a", "b"]);
        let result = f.call(vec![list], AgentHandle::detached()).await.unwrap();
        assert_eq!(result.type_name(), "ListIterator");
    }

    #[tokio::test]
    async fn move_next_native_fn_returns_false_on_empty() {
        let impls = IteratorModule.native_impls();
        let li_impl = impls
            .iter()
            .find(|i| i.type_name == "ListIterator")
            .unwrap();
        let mn_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "move_next")
            .unwrap();
        let f = get_fn_ptr(mn_def);
        let list = make_string_list(vec![]);
        let impls_for_iter = IteratorModule.native_impls();
        let list_impl = impls_for_iter
            .iter()
            .find(|i| i.type_name == "List")
            .unwrap();
        let iter_def = list_impl
            .functions
            .iter()
            .find(|f| f.name == "iter")
            .unwrap();
        let iter_f = get_fn_ptr(iter_def);
        let iter = iter_f
            .call(vec![list], AgentHandle::detached())
            .await
            .unwrap();
        let result = f.call(vec![iter], AgentHandle::detached()).await.unwrap();
        assert_eq!(result.as_boolean().unwrap(), false);
    }

    #[tokio::test]
    async fn move_next_native_fn_returns_true_on_non_empty() {
        let impls = IteratorModule.native_impls();
        let li_impl = impls
            .iter()
            .find(|i| i.type_name == "ListIterator")
            .unwrap();
        let mn_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "move_next")
            .unwrap();
        let f = get_fn_ptr(mn_def);
        let list = make_string_list(vec!["a"]);
        let impls_for_iter = IteratorModule.native_impls();
        let list_impl = impls_for_iter
            .iter()
            .find(|i| i.type_name == "List")
            .unwrap();
        let iter_def = list_impl
            .functions
            .iter()
            .find(|f| f.name == "iter")
            .unwrap();
        let iter_f = get_fn_ptr(iter_def);
        let iter = iter_f
            .call(vec![list], AgentHandle::detached())
            .await
            .unwrap();
        let result = f.call(vec![iter], AgentHandle::detached()).await.unwrap();
        assert_eq!(result.as_boolean().unwrap(), true);
    }

    #[tokio::test]
    async fn current_native_fn_returns_element() {
        let impls = IteratorModule.native_impls();
        let li_impl = impls
            .iter()
            .find(|i| i.type_name == "ListIterator")
            .unwrap();
        let mn_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "move_next")
            .unwrap();
        let cur_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "current")
            .unwrap();
        let mn_f = get_fn_ptr(mn_def);
        let cur_f = get_fn_ptr(cur_def);
        let list = make_string_list(vec!["hello"]);
        let impls_for_iter = IteratorModule.native_impls();
        let list_impl = impls_for_iter
            .iter()
            .find(|i| i.type_name == "List")
            .unwrap();
        let iter_def = list_impl
            .functions
            .iter()
            .find(|f| f.name == "iter")
            .unwrap();
        let iter_f = get_fn_ptr(iter_def);
        let iter = iter_f
            .call(vec![list], AgentHandle::detached())
            .await
            .unwrap();
        mn_f.call(vec![iter.clone()], AgentHandle::detached())
            .await
            .unwrap();
        let result = cur_f
            .call(vec![iter], AgentHandle::detached())
            .await
            .unwrap();
        assert_eq!(result.as_string().unwrap(), "hello");
    }

    #[tokio::test]
    async fn current_native_fn_errors_before_move_next() {
        let impls = IteratorModule.native_impls();
        let li_impl = impls
            .iter()
            .find(|i| i.type_name == "ListIterator")
            .unwrap();
        let cur_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "current")
            .unwrap();
        let cur_f = get_fn_ptr(cur_def);
        let list = make_string_list(vec!["hello"]);
        let impls_for_iter = IteratorModule.native_impls();
        let list_impl = impls_for_iter
            .iter()
            .find(|i| i.type_name == "List")
            .unwrap();
        let iter_def = list_impl
            .functions
            .iter()
            .find(|f| f.name == "iter")
            .unwrap();
        let iter_f = get_fn_ptr(iter_def);
        let iter = iter_f
            .call(vec![list], AgentHandle::detached())
            .await
            .unwrap();
        let result = cur_f.call(vec![iter], AgentHandle::detached()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn full_iteration_via_native_fns() {
        let impls = IteratorModule.native_impls();
        let list_impl = impls.iter().find(|i| i.type_name == "List").unwrap();
        let iter_def = list_impl
            .functions
            .iter()
            .find(|f| f.name == "iter")
            .unwrap();
        let iter_f = get_fn_ptr(iter_def);
        let li_impl = impls
            .iter()
            .find(|i| i.type_name == "ListIterator")
            .unwrap();
        let mn_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "move_next")
            .unwrap();
        let cur_def = li_impl
            .functions
            .iter()
            .find(|f| f.name == "current")
            .unwrap();
        let mn_f = get_fn_ptr(mn_def);
        let cur_f = get_fn_ptr(cur_def);
        let list = make_string_list(vec!["x", "y", "z"]);
        let iter = iter_f
            .call(vec![list], AgentHandle::detached())
            .await
            .unwrap();
        let mut results = vec![];
        while mn_f
            .call(vec![iter.clone()], AgentHandle::detached())
            .await
            .unwrap()
            .as_boolean()
            .unwrap()
        {
            let val = cur_f
                .call(vec![iter.clone()], AgentHandle::detached())
                .await
                .unwrap();
            results.push(val.as_string().unwrap());
        }
        assert_eq!(results, vec!["x", "y", "z"]);
    }
}
