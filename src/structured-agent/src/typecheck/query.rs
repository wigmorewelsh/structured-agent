use super::db::{
    InternedFunctionName, InternedString, InternedTraitName, InternedTypeName, SymbolTablesInput,
    TypeCheckDatabase, find_trait_for_impl_call, lookup_function_def, lookup_impl_exists,
    lookup_trait_def, lookup_type_def,
};
use super::refs::{AliasToQualified, CheckerAstRef, CheckerRefs, FunctionKind};
use super::{CheckContext, FunctionSignature, TypeChecker, TypeEnvironment};
use crate::ast::{Definition, Expression, Module, SigFunction, Type as AstType};
use crate::typecheck::error::TypeError;
use crate::types::Span;
use nonempty::NonEmpty;
use std::collections::HashMap;
use structured_agent_runtime::symbols::{
    FunctionDefinition, FunctionName, FunctionNameKind, MetaData, ModuleName, SymbolQuery,
    TraitName, TypeName, UseImport, Visibility,
};

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
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
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
                    module: ModuleName::new(path.clone()),
                    name: name.clone(),
                };
                let type_name = TypeName {
                    name: import.name.clone(),
                    module: import.module.clone(),
                };
                if tables.types(db).get().get(&type_name).is_some() {
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

pub(super) fn build_alias_to_qualified(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: &Module,
) -> AliasToQualified {
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
        let interned = InternedFunctionName::new(db, fn_key);
        let fn_exists = lookup_function_def(db, tables, interned).is_some();
        if fn_exists {
            map.insert(import.local.clone(), import);
        }
    }
    map
}

pub(super) fn get_function_sig(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &FunctionName,
    type_imports: &HashMap<String, UseImport>,
) -> Option<FunctionSignature> {
    let make_sig = |f: &FunctionDefinition<CheckerRefs>| match &f.ast_ref {
        CheckerAstRef::Function(func, kind) => Some(FunctionSignature {
            parameters: func
                .parameters
                .iter()
                .map(|p| crate::ast::Parameter {
                    name: p.name.clone(),
                    param_type: super::constraints::resolve_type(
                        db,
                        tables,
                        &p.param_type,
                        &name.module,
                        type_imports,
                    ),
                    span: p.span,
                })
                .collect(),
            return_type: super::constraints::resolve_type(
                db,
                tables,
                &func.return_type,
                &name.module,
                type_imports,
            ),
            type_params: func.type_params.clone(),
            kind: kind.clone(),
        }),
        CheckerAstRef::ImplFunction(func, concrete_type, kind) => Some(FunctionSignature {
            parameters: func
                .parameters
                .iter()
                .map(|p| crate::ast::Parameter {
                    name: p.name.clone(),
                    param_type: super::constraints::resolve_type(
                        db,
                        tables,
                        &TypeChecker::substitute_self(&p.param_type, concrete_type),
                        &name.module,
                        type_imports,
                    ),
                    span: p.span,
                })
                .collect(),
            return_type: super::constraints::resolve_type(
                db,
                tables,
                &TypeChecker::substitute_self(&func.return_type, concrete_type),
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
    let key = InternedFunctionName::new(db, name.clone());
    lookup_function_def(db, tables, key).and_then(|arc_ptr| make_sig(arc_ptr.get()))
}

pub(super) fn get_struct_fields(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    current_module: &ModuleName,
    type_imports: &HashMap<String, UseImport>,
) -> Option<Vec<(String, AstType)>> {
    let resolved = TypeChecker::resolve_named_type(name, current_module, type_imports);
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
    let key = InternedTypeName::new(db, resolved);
    lookup_type_def(db, tables, key).and_then(|arc_ptr| extract(&arc_ptr.get().ast_ref))
}

pub(super) fn get_trait_functions(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    current_module: &ModuleName,
    type_imports: &HashMap<String, UseImport>,
) -> Option<Vec<SigFunction>> {
    let resolved = TypeChecker::resolve_named_type(name, current_module, type_imports);
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
    let key = InternedTraitName::new(db, trait_name);
    lookup_trait_def(db, tables, key).and_then(|arc_ptr| extract(&arc_ptr.get().ast_ref))
}

pub(super) fn type_implements_trait(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    type_name: &str,
    trait_name: &str,
) -> bool {
    let tn = TypeName {
        name: type_name.to_string(),
        module: ModuleName::new(NonEmpty::new(String::new())),
    };
    let trn = TraitName {
        name: trait_name.to_string(),
        module: ModuleName::new(NonEmpty::new(String::new())),
    };
    let type_key = InternedTypeName::new(db, tn);
    let trait_key = InternedTraitName::new(db, trn);
    lookup_impl_exists(db, tables, type_key, trait_key)
}

pub(super) fn resolve_impl_call(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    fn_name: &str,
    arguments: &[Expression],
    env: &TypeEnvironment,
    ctx: &CheckContext,
) -> Option<(FunctionName, FunctionSignature)> {
    if arguments.is_empty() {
        return None;
    }
    let first_arg =
        super::elaboration::check_expression(db, tables, &arguments[0], env, ctx).ok()?;
    let type_name = match first_arg.ty() {
        AstType::Int => "Int".to_string(),
        AstType::String => "String".to_string(),
        AstType::Boolean => "Boolean".to_string(),
        AstType::Struct(n) => n.clone(),
        _ => return None,
    };
    let interned_fn = InternedString::new(db, fn_name.to_string());
    let interned_type = InternedTypeName::new(
        db,
        TypeName {
            name: type_name.clone(),
            module: ModuleName::new(NonEmpty::new(String::new())),
        },
    );
    if let Some(interned_trait) = find_trait_for_impl_call(db, tables, interned_fn, interned_type) {
        let trait_key_name = interned_trait.name(db);
        let impl_fn_name = {
            let mn = ctx.module_name.clone();
            FunctionName {
                name: fn_name.to_string(),
                module: mn.clone(),
                kind: FunctionNameKind::Impl {
                    type_name: TypeName {
                        name: type_name.to_string(),
                        module: mn.clone(),
                    },
                    trait_name: TraitName {
                        name: trait_key_name.name.to_string(),
                        module: mn,
                    },
                },
            }
        };
        if let Some(sig) = get_function_sig(db, tables, &impl_fn_name, ctx.type_imports) {
            return Some((impl_fn_name, sig));
        }
    }
    None
}

pub(super) fn check_visibility(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
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
            module: ModuleName::new(
                NonEmpty::from_vec(module_part.split("::").map(|s| s.to_string()).collect())
                    .unwrap(),
            ),
            kind: FunctionNameKind::Function,
        },
        None => return Ok(()),
    };

    let interned = InternedFunctionName::new(db, fn_key);
    let is_visible = lookup_function_def(db, tables, interned)
        .map(|arc_ptr| matches!(arc_ptr.get().visibility, Visibility::Public))
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

pub(super) fn lookup_sig(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    resolved: &str,
    ctx: &CheckContext,
) -> Option<FunctionSignature> {
    if resolved.contains("::") {
        let name = match resolved.rsplit_once("::") {
            Some((module, name)) => FunctionName {
                name: name.to_string(),
                module: ModuleName::new(
                    NonEmpty::from_vec(module.split("::").map(|s| s.to_string()).collect())
                        .unwrap(),
                ),
                kind: FunctionNameKind::Function,
            },
            None => FunctionName {
                name: resolved.to_string(),
                module: ModuleName::new(NonEmpty::new(String::new())),
                kind: FunctionNameKind::Function,
            },
        };
        get_function_sig(db, tables, &name, ctx.type_imports)
    } else {
        if let Some(import) = ctx.alias_to_qualified.get(resolved) {
            let name = FunctionName {
                name: import.name.clone(),
                module: import.module.clone(),
                kind: FunctionNameKind::Function,
            };
            if let Some(sig) = get_function_sig(db, tables, &name, ctx.type_imports) {
                return Some(sig);
            }
        }
        let name = FunctionName {
            name: resolved.to_string(),
            module: ctx.module_name.clone(),
            kind: FunctionNameKind::Function,
        };
        get_function_sig(db, tables, &name, ctx.type_imports).or_else(|| {
            let fallback = FunctionName {
                name: resolved.to_string(),
                module: ModuleName::new(NonEmpty::new(String::new())),
                kind: FunctionNameKind::Function,
            };
            get_function_sig(db, tables, &fallback, ctx.type_imports)
        })
    }
}

impl TypeChecker {
    fn get_function_sig(
        &self,
        name: &FunctionName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<FunctionSignature> {
        get_function_sig(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            name,
            type_imports,
        )
    }

    pub(super) fn get_struct_fields(
        &self,
        name: &str,
        current_module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<Vec<(String, AstType)>> {
        get_struct_fields(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            name,
            current_module,
            type_imports,
        )
    }

    pub(super) fn get_trait_functions(
        &self,
        name: &str,
        current_module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Option<Vec<SigFunction>> {
        get_trait_functions(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            name,
            current_module,
            type_imports,
        )
    }

    pub(super) fn type_implements_trait(&self, type_name: &str, trait_name: &str) -> bool {
        type_implements_trait(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            type_name,
            trait_name,
        )
    }

    pub(super) fn resolve_impl_call(
        &self,
        fn_name: &str,
        arguments: &[Expression],
        env: &super::TypeEnvironment,
        ctx: &CheckContext,
    ) -> Option<(FunctionName, FunctionSignature)> {
        resolve_impl_call(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            fn_name,
            arguments,
            env,
            ctx,
        )
    }

    pub(super) fn build_alias_map(module: &Module) -> HashMap<String, String> {
        build_alias_map(module)
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

    pub(super) fn resolve_named_type(
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
        build_alias_to_qualified(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            module,
        )
    }

    pub(super) fn check_visibility(
        &self,
        qualified_for_vis: &str,
        resolved: &str,
        span: Span,
        ctx: &CheckContext,
    ) -> Result<(), TypeError> {
        check_visibility(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            qualified_for_vis,
            resolved,
            span,
            ctx,
        )
    }

    pub(super) fn lookup_sig(
        &self,
        resolved: &str,
        ctx: &CheckContext,
    ) -> Option<FunctionSignature> {
        lookup_sig(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            resolved,
            ctx,
        )
    }

    pub(super) fn make_function_name(
        resolved: &str,
        ctx: &CheckContext,
        kind: &FunctionKind,
    ) -> FunctionName {
        let from_qual = |s: &str| match s.rsplit_once("::") {
            Some((module, name)) => FunctionName {
                name: name.to_string(),
                module: ModuleName::new(
                    NonEmpty::from_vec(module.split("::").map(|s| s.to_string()).collect())
                        .unwrap(),
                ),
                kind: FunctionNameKind::Function,
            },
            None => FunctionName {
                name: s.to_string(),
                module: ModuleName::new(NonEmpty::new(String::new())),
                kind: FunctionNameKind::Function,
            },
        };
        if *kind == FunctionKind::External {
            return from_qual(resolved);
        }
        if let Some(import) = ctx.alias_to_qualified.get(resolved) {
            FunctionName {
                name: import.name.clone(),
                module: import.module.clone(),
                kind: FunctionNameKind::Function,
            }
        } else if resolved.contains("::") {
            from_qual(resolved)
        } else {
            FunctionName {
                name: resolved.to_string(),
                module: ctx.module_name.clone(),
                kind: FunctionNameKind::Function,
            }
        }
    }
}
