use super::refs::{AliasToQualified, CheckerAstRef, FunctionKind};
use super::{CheckContext, FunctionSignature, TypeChecker};
use crate::ast::{Definition, Expression, Module, SigFunction, Type as AstType};
use crate::typecheck::error::TypeError;
use crate::types::Span;
use std::collections::HashMap;
use structured_agent_runtime::symbols::{
    FunctionName, FunctionNameKind, ModuleName, SymbolQuery, TypeDefinitionKind, Visibility,
};

impl TypeChecker {
    pub(super) fn get_function_sig(&self, name: &FunctionName) -> Option<FunctionSignature> {
        self.metadata.function(name).and_then(|f| match &f.ast_ref {
            CheckerAstRef::Function(func, kind) => Some(FunctionSignature {
                parameters: func
                    .parameters
                    .iter()
                    .map(|p| crate::ast::Parameter {
                        name: p.name.clone(),
                        param_type: self.resolve_type(&p.param_type),
                        span: p.span,
                    })
                    .collect(),
                return_type: self.resolve_type(&func.return_type),
                type_params: func.type_params.clone(),
                kind: kind.clone(),
            }),
            CheckerAstRef::ImplFunction(func, concrete_type, kind) => Some(FunctionSignature {
                parameters: func
                    .parameters
                    .iter()
                    .map(|p| crate::ast::Parameter {
                        name: p.name.clone(),
                        param_type: self
                            .resolve_type(&Self::substitute_self(&p.param_type, concrete_type)),
                        span: p.span,
                    })
                    .collect(),
                return_type: self
                    .resolve_type(&Self::substitute_self(&func.return_type, concrete_type)),
                type_params: func.type_params.clone(),
                kind: kind.clone(),
            }),
            CheckerAstRef::ExternalFn {
                params,
                return_type,
                type_params,
                kind,
            } => Some(FunctionSignature {
                parameters: params.clone(),
                return_type: return_type.clone(),
                type_params: type_params.clone(),
                kind: kind.clone(),
            }),
            _ => None,
        })
    }

    pub(super) fn get_struct_fields(&self, name: &str) -> Option<Vec<(String, AstType)>> {
        self.metadata.type_by_name(name).and_then(|td| {
            if let CheckerAstRef::Struct(s) = &td.ast_ref {
                Some(
                    s.fields
                        .iter()
                        .map(|f| (f.name.clone(), f.field_type.clone()))
                        .collect(),
                )
            } else {
                None
            }
        })
    }

    pub(super) fn get_trait_functions(&self, name: &str) -> Option<Vec<crate::ast::SigFunction>> {
        self.metadata.trait_by_name(name).and_then(|td| {
            if let CheckerAstRef::Trait(t) = &td.ast_ref {
                Some(t.functions.clone())
            } else {
                None
            }
        })
    }

    pub(super) fn type_implements_trait(&self, type_name: &str, trait_name: &str) -> bool {
        self.metadata
            .impls
            .keys()
            .any(|k| k.type_name.name == type_name && k.trait_name.name == trait_name)
            || self
                .param_bindings
                .keys()
                .any(|k| k.type_name.name == type_name && k.trait_name.name == trait_name)
    }

    pub(super) fn get_sig_functions(&self, name: &str) -> Option<Vec<SigFunction>> {
        self.metadata
            .type_by_name(name)
            .filter(|td| matches!(td.kind, TypeDefinitionKind::Signature { .. }))
            .and_then(|td| {
                if let CheckerAstRef::Signature(s) = &td.ast_ref {
                    Some(s.functions.clone())
                } else {
                    None
                }
            })
    }

    pub(super) fn resolve_impl_call(
        &self,
        fn_name: &str,
        arguments: &[Expression],
        env: &super::TypeEnvironment,
        ctx: &CheckContext,
    ) -> Option<(FunctionName, FunctionSignature)> {
        if arguments.is_empty() {
            return None;
        }
        let first_arg = self.check_expression(&arguments[0], env, ctx).ok()?;
        let type_name = match first_arg.ty() {
            AstType::Int => "Int".to_string(),
            AstType::String => "String".to_string(),
            AstType::Boolean => "Boolean".to_string(),
            AstType::Struct(n) => n.clone(),
            _ => return None,
        };
        for (trait_key, trait_def) in &self.metadata.traits {
            let CheckerAstRef::Trait(trait_fns) = &trait_def.ast_ref else {
                continue;
            };
            if trait_fns.functions.iter().any(|f| f.name == fn_name)
                && self.type_implements_trait(&type_name, &trait_key.name)
            {
                let module = ctx.module_name.unwrap_or("");
                let impl_fn_name = {
                    let mn = ModuleName::from_str(module);
                    FunctionName {
                        name: fn_name.to_string(),
                        module: mn.clone(),
                        kind: FunctionNameKind::Impl {
                            type_name: structured_agent_runtime::symbols::TypeName {
                                name: type_name.to_string(),
                                module: mn.clone(),
                            },
                            trait_name: structured_agent_runtime::symbols::TraitName {
                                name: trait_key.name.to_string(),
                                module: mn,
                            },
                        },
                    }
                };
                if let Some(sig) = self.get_function_sig(&impl_fn_name) {
                    return Some((impl_fn_name, sig));
                }
            }
        }
        None
    }

    pub(super) fn build_alias_map(module: &Module) -> HashMap<String, String> {
        module
            .definitions
            .iter()
            .filter_map(|def| {
                if let Definition::Use {
                    path,
                    alias: Some(a),
                    ..
                } = def
                {
                    Some((a.clone(), path.last().cloned().unwrap_or_default()))
                } else {
                    None
                }
            })
            .collect()
    }

    #[allow(deprecated)]
    pub(super) fn build_alias_to_qualified(&self, module: &Module) -> AliasToQualified {
        let mut map = AliasToQualified::new();

        for def in &module.definitions {
            let Definition::Use { path, alias, .. } = def else {
                continue;
            };
            if path.len() < 2 {
                continue;
            }

            let dep_module = &path[0];
            let fn_name = path.last().unwrap();
            let qualified = format!("{}::{}", dep_module, fn_name);

            let fn_key = match qualified.rsplit_once("::") {
                Some((module_part, name)) => FunctionName {
                    name: name.to_string(),
                    module: ModuleName::from_str(module_part),
                    kind: FunctionNameKind::Function,
                },
                None => FunctionName {
                    name: qualified.clone(),
                    module: ModuleName::unqualified(),
                    kind: FunctionNameKind::Function,
                },
            };
            if self.metadata.function(&fn_key).is_some() {
                let key = alias.clone().unwrap_or_else(|| fn_name.clone());
                map.insert(key, qualified);
            }
        }

        map
    }

    pub(super) fn check_visibility(
        &self,
        qualified_for_vis: &str,
        resolved: &str,
        span: Span,
        ctx: &CheckContext,
    ) -> Result<(), TypeError> {
        let name_to_check = if qualified_for_vis.contains("::") {
            qualified_for_vis
        } else if resolved.contains("::") {
            resolved
        } else {
            return Ok(());
        };

        let fn_key = match name_to_check.rsplit_once("::") {
            Some((module_part, name)) => FunctionName {
                name: name.to_string(),
                module: ModuleName::from_str(module_part),
                kind: FunctionNameKind::Function,
            },
            None => return Ok(()),
        };

        let is_visible = self
            .metadata
            .function(&fn_key)
            .map(|f| matches!(f.visibility, Visibility::Public))
            .unwrap_or(true);

        if is_visible {
            Ok(())
        } else {
            Err(TypeError::PrivateFunction {
                name: name_to_check.to_string(),
                span,
                file_id: ctx.file_id,
            })
        }
    }

    #[allow(deprecated)]
    pub(super) fn lookup_sig(
        &self,
        resolved: &str,
        ctx: &CheckContext,
    ) -> Option<FunctionSignature> {
        if resolved.contains("::") {
            let name = match resolved.rsplit_once("::") {
                Some((module, name)) => FunctionName {
                    name: name.to_string(),
                    module: ModuleName::from_str(module),
                    kind: FunctionNameKind::Function,
                },
                None => FunctionName {
                    name: resolved.to_string(),
                    module: ModuleName::unqualified(),
                    kind: FunctionNameKind::Function,
                },
            };
            self.get_function_sig(&name)
        } else {
            if let Some(qualified) = ctx.alias_to_qualified.get(resolved) {
                let name = match qualified.rsplit_once("::") {
                    Some((module, name)) => FunctionName {
                        name: name.to_string(),
                        module: ModuleName::from_str(module),
                        kind: FunctionNameKind::Function,
                    },
                    None => FunctionName {
                        name: qualified.to_string(),
                        module: ModuleName::unqualified(),
                        kind: FunctionNameKind::Function,
                    },
                };
                if let Some(sig) = self.get_function_sig(&name) {
                    return Some(sig);
                }
            }
            let module = ctx.module_name.unwrap_or("");
            let name = FunctionName {
                name: resolved.to_string(),
                module: ModuleName::from_str(module),
                kind: FunctionNameKind::Function,
            };
            self.get_function_sig(&name).or_else(|| {
                let fallback = FunctionName {
                    name: resolved.to_string(),
                    module: ModuleName::unqualified(),
                    kind: FunctionNameKind::Function,
                };
                self.get_function_sig(&fallback)
            })
        }
    }

    #[allow(deprecated)]
    pub(super) fn make_function_name(
        resolved: &str,
        ctx: &CheckContext,
        kind: &FunctionKind,
    ) -> FunctionName {
        let from_str = |s: &str| match s.rsplit_once("::") {
            Some((module, name)) => FunctionName {
                name: name.to_string(),
                module: ModuleName::from_str(module),
                kind: FunctionNameKind::Function,
            },
            None => FunctionName {
                name: s.to_string(),
                module: ModuleName::unqualified(),
                kind: FunctionNameKind::Function,
            },
        };
        let plain = |module: &str, name: &str| FunctionName {
            name: name.to_string(),
            module: ModuleName::from_str(module),
            kind: FunctionNameKind::Function,
        };
        if *kind == FunctionKind::External {
            return from_str(resolved);
        }
        if let Some(qualified) = ctx.alias_to_qualified.get(resolved) {
            from_str(qualified)
        } else if resolved.contains("::") {
            from_str(resolved)
        } else if let Some(module) = ctx.module_name {
            if module.is_empty() {
                plain("", resolved)
            } else {
                plain(module, resolved)
            }
        } else {
            plain("", resolved)
        }
    }
}
