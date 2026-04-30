use structured_agent_macros::sa_module;

#[sa_module]
pub mod actor {
    use structured_agent_runtime::StringValue;

    #[sa_fn]
    async fn actor_id() -> StringValue {
        agent.actor_id().unwrap_or("").to_string().into()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use structured_agent_il::{Instruction, Module};
        use structured_agent_runtime::{AgentHandle, ExpressionValue, NativeFnPtr};

        fn get_fn_ptr(def: &structured_agent_il::NativeFunctionDef) -> NativeFnPtr {
            if let Instruction::CallNative { f, .. } = &def.body[0] {
                f.clone()
            } else {
                panic!("expected CallNative instruction");
            }
        }

        #[test]
        fn actor_id_def_has_correct_name_and_signature() {
            let def = actor_id_native_def();
            assert_eq!(def.name, "actor_id");
            assert_eq!(def.parameters.len(), 0);
            assert_eq!(def.return_type.name(), "String");
        }

        #[tokio::test]
        async fn actor_id_returns_empty_string_outside_actor_context() {
            let def = actor_id_native_def();
            let f = get_fn_ptr(&def);
            let result = f.call(vec![], AgentHandle::detached()).await.unwrap();
            assert_eq!(result, ExpressionValue::string(""));
        }

        #[tokio::test]
        async fn actor_id_returns_set_id() {
            let def = actor_id_native_def();
            let f = get_fn_ptr(&def);
            let handle = AgentHandle::detached().with_actor_id("Counter:my_actor".to_string());
            let result = f.call(vec![], handle).await.unwrap();
            assert_eq!(result, ExpressionValue::string("Counter:my_actor"));
        }

        #[test]
        fn actor_module_exposes_actor_id_function() {
            let module = ActorModule;
            let fns = module.native_functions();
            assert_eq!(fns.len(), 1);
            assert_eq!(fns[0].name, "actor_id");
        }
    }
}

pub use actor::ActorModule;
