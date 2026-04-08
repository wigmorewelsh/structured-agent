use super::db::{
    InternedFunctionName, InternedTraitName, InternedTypeName, lookup_function_def,
    lookup_impl_exists, lookup_trait_def, lookup_type_def,
};
use super::refs::{AliasToQualified, CheckerAstRef, CheckerRefs, FunctionKind};
use super::{CheckContext, FunctionSignature, TypeChecker};
use crate::ast::{Definition, Expression, Module, SigFunction, Type as AstType};
use crate::typecheck::error::TypeError;
use crate::types::Span;
use std::collections::HashMap;
use structured_agent_runtime::symbols::{
    FunctionName, FunctionNameKind, MetaData, ModuleName, SymbolQuery, TraitName,
    TypeDefinitionKind, TypeName, UseImport, Visibility,
};

impl TypeChecker {
    fn get_function_sig(
        &self,
        name: &FunctionName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<FunctionSignature> {
        let make_sig =
            |f: &structured_agent_runtime::symbols::FunctionDefinition<CheckerRefs>| match &f
                .ast_ref
            {
                CheckerAstRef::Function(func, kind) => Some(FunctionSignature {
                    parameters: func
                        .parameters
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: self.resolve_type(
                                &p.param_type,
                                &name.module,
                                type_imports,
                            ),
                            span: p.span,
                        })
                        .collect(),
                    return_type: self.resolve_type(&func.return_type, &name.module, type_imports),
                    type_params: func.type_params.clone(),
                    kind: kind.clone(),
                }),
                CheckerAstRef::ImplFunction(func, concrete_type, kind) => Some(FunctionSignature {
                    parameters: func
                        .parameters
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: self.resolve_type(
                                &Self::substitute_self(&p.param_type, concrete_type),
                                &name.module,
                                type_imports,
                            ),
                            span: p.span,
                        })
                        .collect(),
                    return_type: self.resolve_type(
                        &Self::substitute_self(&func.return_type, concrete_type),
                        &name.module,
                        type_imports,
                    ),
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
            };
        if let Some(tables) = self.symbol_tables {
            let key = InternedFunctionName::new(&self.db, name.clone());
            lookup_function_def(&self.db, tables, key).and_then(|arc_ptr| make_sig(arc_ptr.get()))
        } else {
            self.metadata.function(name).and_then(|f| make_sig(&*f))
        }
    }

    pub(super) fn get_struct_fields(
        &self,
        name: &str,
        current_module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<Vec<(String, AstType)>> {
        let resolved = Self::resolve_named_type(name, current_module, type_imports);
        let extract = |ast_ref: &CheckerAstRef| {
            if let CheckerAstRef::Struct(s) = ast_ref {
                Some(
                    s.fields
                        .iter()
                        .map(|f| (f.name.clone(), f.field_type.clone()))
                        .collect(),
                )
            } else {
                None
            }
        };
        if let Some(tables) = self.symbol_tables {
            let key = InternedTypeName::new(&self.db, resolved);
            lookup_type_def(&self.db, tables, key)
                .and_then(|arc_ptr| extract(&arc_ptr.get().ast_ref))
        } else {
            self.metadata
                .type_def(&resolved)
                .and_then(|td| extract(&td.ast_ref))
        }
    }

    pub(super) fn get_trait_functions(
        &self,
        name: &str,
        current_module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<Vec<SigFunction>> {
        let resolved = Self::resolve_named_type(name, current_module, type_imports);
        let trait_name = TraitName {
            name: resolved.name,
            module: resolved.module,
        };
        let extract = |ast_ref: &CheckerAstRef| {
            if let CheckerAstRef::Trait(t) = ast_ref {
                Some(t.functions.clone())
            } else {
                None
            }
        };
        if let Some(tables) = self.symbol_tables {
            let key = InternedTraitName::new(&self.db, trait_name);
            lookup_trait_def(&self.db, tables, key)
                .and_then(|arc_ptr| extract(&arc_ptr.get().ast_ref))
        } else {
            self.metadata
                .trait_def(&trait_name)
                .and_then(|td| extract(&td.ast_ref))
        }
    }

    pub(super) fn type_implements_trait(&self, type_name: &str, trait_name: &str) -> bool {
        if let Some(tables) = self.symbol_tables {
            let tn = TypeName {
                name: type_name.to_string(),
                module: ModuleName::unqualified(),
            };
            let trn = TraitName {
                name: trait_name.to_string(),
                module: ModuleName::unqualified(),
            };
            let type_key = InternedTypeName::new(&self.db, tn);
            let trait_key = InternedTraitName::new(&self.db, trn);
            lookup_impl_exists(&self.db, tables, type_key, trait_key)
        } else {
            self.metadata
                .impls
                .keys()
                .any(|k| k.type_name.name == type_name && k.trait_name.name == trait_name)
                || self
                    .param_bindings
                    .keys()
                    .any(|k| k.type_name.name == type_name && k.trait_name.name == trait_name)
        }
    }

    pub(super) fn get_sig_functions(
        &self,
        name: &str,
        current_module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<Vec<SigFunction>> {
        let resolved = Self::resolve_named_type(name, current_module, type_imports);
        let extract = |td_kind: &TypeDefinitionKind, ast_ref: &CheckerAstRef| {
            if !matches!(td_kind, TypeDefinitionKind::Signature { .. }) {
                return None;
            }
            if let CheckerAstRef::Signature(s) = ast_ref {
                Some(s.functions.clone())
            } else {
                None
            }
        };
        if let Some(tables) = self.symbol_tables {
            let key = InternedTypeName::new(&self.db, resolved);
            lookup_type_def(&self.db, tables, key)
                .and_then(|arc_ptr| extract(&arc_ptr.get().kind, &arc_ptr.get().ast_ref))
        } else {
            self.metadata
                .type_def(&resolved)
                .and_then(|td| extract(&td.kind, &td.ast_ref))
        }
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
                if let Some(sig) = self.get_function_sig(&impl_fn_name, ctx.type_imports) {
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
                    name,
                    alias: Some(a),
                    ..
                } = def
                {
                    Some((a.clone(), name.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    pub(super) fn build_type_import_map(
        module: &Module,
        metadata: &MetaData<CheckerRefs>,
    ) -> HashMap<String, UseImport> {
        module
            .definitions
            .iter()
            .filter_map(|def| {
                if let Definition::Use {
                    path, name, alias, ..
                } = def
                {
                    let import = UseImport {
                        local: alias.clone().unwrap_or_else(|| name.clone()),
                        // FIXME: this is wrong them module is the current.module + path, the current module is a arg
                        module: ModuleName::new(path.clone()),
                        name: name.clone(),
                    };
                    if metadata
                        .type_def(&TypeName {
                            name: import.name.clone(),
                            module: import.module.clone(),
                        })
                        .is_some()
                    {
                        Some((import.local.clone(), import))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    fn resolve_named_type(
        name: &str,
        current_module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> TypeName {
        if let Some(import) = type_imports.get(name) {
            TypeName {
                name: import.name.clone(),
                module: import.module.clone(),
            }
        } else {
            TypeName {
                name: name.to_string(),
                module: current_module.clone(),
            }
        }
    }

    pub(super) fn build_alias_to_qualified(&self, module: &Module) -> AliasToQualified {
        let mut map = AliasToQualified::new();

        for def in &module.definitions {
            let Definition::Use {
                path, name, alias, ..
            } = def
            else {
                continue;
            };

            let import = UseImport {
                local: alias.clone().unwrap_or_else(|| name.clone()),
                module: ModuleName::new(path.clone()),
                name: name.clone(),
            };
            let fn_key = FunctionName {
                name: import.name.clone(),
                module: import.module.clone(),
                kind: FunctionNameKind::Function,
            };
            let fn_exists = if let Some(tables) = self.symbol_tables {
                let interned = InternedFunctionName::new(&self.db, fn_key.clone());
                lookup_function_def(&self.db, tables, interned).is_some()
            } else {
                self.metadata.function(&fn_key).is_some()
            };
            if fn_exists {
                map.insert(import.local.clone(), import);
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

        let is_visible = if let Some(tables) = self.symbol_tables {
            let interned = InternedFunctionName::new(&self.db, fn_key.clone());
            lookup_function_def(&self.db, tables, interned)
                .map(|arc_ptr| matches!(arc_ptr.get().visibility, Visibility::Public))
                .unwrap_or(true)
        } else {
            self.metadata
                .function(&fn_key)
                .map(|f| matches!(f.visibility, Visibility::Public))
                .unwrap_or(true)
        };

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
            self.get_function_sig(&name, ctx.type_imports)
        } else {
            if let Some(import) = ctx.alias_to_qualified.get(resolved) {
                let name = FunctionName {
                    name: import.name.clone(),
                    module: import.module.clone(),
                    kind: FunctionNameKind::Function,
                };
                if let Some(sig) = self.get_function_sig(&name, ctx.type_imports) {
                    return Some(sig);
                }
            }
            let module = ctx.module_name.unwrap_or("");
            let name = FunctionName {
                name: resolved.to_string(),
                module: ModuleName::from_str(module),
                kind: FunctionNameKind::Function,
            };
            self.get_function_sig(&name, ctx.type_imports).or_else(|| {
                let fallback = FunctionName {
                    name: resolved.to_string(),
                    module: ModuleName::unqualified(),
                    kind: FunctionNameKind::Function,
                };
                self.get_function_sig(&fallback, ctx.type_imports)
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
        if let Some(import) = ctx.alias_to_qualified.get(resolved) {
            FunctionName {
                name: import.name.clone(),
                module: import.module.clone(),
                kind: FunctionNameKind::Function,
            }
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
