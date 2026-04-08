use super::refs::{CheckerAstRef, FunctionKind, NoWitness, SourceLocation};
use super::{TypeChecker, ast_type_to_type_name};
use crate::ast::{Definition, Module, Parameter, ParsedModule, Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span};
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    FieldDefinition, FunctionDefinition, FunctionName, FunctionNameKind,
    GenericParameterDefinition, ImplDefinition, ImplKey, ModuleName, NoAst, ParameterDefinition,
    SignatureEntry, SymbolQuery, TraitDefinition, TraitName, TypeDefinition, TypeDefinitionKind,
    TypeName, Visibility,
};
use structured_agent_runtime::types::Module as RuntimeModule;

impl TypeChecker {
    pub(super) fn seed_builtin_types(&mut self) {
        let builtins = [
            ("()", "prelude"),
            ("Boolean", "prelude"),
            ("String", "prelude"),
            ("Int", "prelude"),
            ("List", "prelude"),
            ("Option", "prelude"),
        ];
        for (name, module) in builtins {
            let type_name = TypeName {
                name: name.to_string(),
                module: ModuleName::from_str(module),
            };
            let entry = TypeDefinition {
                name: type_name.clone(),
                kind: TypeDefinitionKind::Primitive,
                source_ref: SourceLocation(0, crate::types::Span::dummy()),
                ast_ref: NoAst,
            };
            self.primitive_types.insert(type_name, Arc::new(entry));
        }
    }

    #[allow(deprecated)]
    pub(super) fn collect_native_sigs(
        &mut self,
        parsed: &ParsedModule,
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) {
        for def in &parsed.module.definitions {
            let Definition::Use { path, .. } = def else {
                continue;
            };
            if path.len() < 2 {
                continue;
            }
            let native_mod_name = &path[0];
            let fn_name = path.last().unwrap();
            let Some(native_mod) = native_modules.get(native_mod_name) else {
                continue;
            };
            let Some(func) = native_mod
                .functions()
                .into_iter()
                .find(|f| f.name() == fn_name)
            else {
                continue;
            };
            let qname = format!("{}::{}", native_mod_name, fn_name);
            let parameters = func
                .parameters()
                .iter()
                .map(|p| Parameter {
                    name: p.name.clone(),
                    param_type: Self::runtime_type_to_ast(&p.param_type),
                    span: Span::dummy(),
                })
                .collect();
            let return_type = Self::runtime_type_to_ast(func.return_type());
            let type_params = func
                .type_params()
                .iter()
                .map(|s| TypeParam::from(s.as_str()))
                .collect();
            let fn_key = match qname.rsplit_once("::") {
                Some((module, name)) => FunctionName {
                    name: name.to_string(),
                    module: ModuleName::from_str(module),
                    kind: FunctionNameKind::Function,
                },
                None => FunctionName {
                    name: qname.to_string(),
                    module: ModuleName::unqualified(),
                    kind: FunctionNameKind::Function,
                },
            };
            self.insert_fn(
                fn_key,
                parameters,
                return_type,
                type_params,
                FunctionKind::External,
                Visibility::Public,
                SourceLocation(parsed.file_id, Span::dummy()),
            );
        }
    }

    fn runtime_type_to_ast(ty: &structured_agent_runtime::types::Type) -> AstType {
        use structured_agent_runtime::types::Type as RT;
        match ty {
            RT::String => AstType::String,
            RT::Boolean => AstType::Boolean,
            RT::Int => AstType::Int,
            RT::Unit => AstType::Unit,
            RT::List(inner) => AstType::List(Box::new(Self::runtime_type_to_ast(inner))),
            RT::Option(inner) => AstType::Option(Box::new(Self::runtime_type_to_ast(inner))),
            RT::Struct(name) => AstType::Struct(name.clone()),
            RT::Generic(name) => AstType::Generic(name.clone()),
        }
    }

    fn insert_fn(
        &mut self,
        name: FunctionName,
        params: Vec<crate::ast::Parameter>,
        return_type: AstType,
        type_params: Vec<TypeParam>,
        kind: FunctionKind,
        visibility: Visibility,
        source_ref: SourceLocation,
    ) {
        let fn_type_name = TypeName {
            name: name.name.clone(),
            module: name.module.clone(),
        };
        let return_type_name = ast_type_to_type_name(&return_type, &name.module.to_string());
        let parameters: Vec<ParameterDefinition> = params
            .iter()
            .map(|p| ParameterDefinition {
                name: p.name.clone(),
                type_name: ast_type_to_type_name(&p.param_type, &name.module.to_string()),
            })
            .collect();
        let generic_parameters: Vec<GenericParameterDefinition> = type_params
            .iter()
            .map(|tp| GenericParameterDefinition {
                name: tp.name.clone(),
                constraints: tp
                    .bounds
                    .iter()
                    .map(|b| TraitName {
                        name: b.clone(),
                        module: name.module.clone(),
                    })
                    .collect(),
            })
            .collect();
        let entry = FunctionDefinition {
            name: name.clone(),
            visibility,
            type_name: fn_type_name.clone(),
            source_ref: SourceLocation(source_ref.0, source_ref.1),
            ast_ref: CheckerAstRef::ExternalFn {
                params: params.clone(),
                return_type: return_type.clone(),
                type_params: type_params.clone(),
                kind: kind.clone(),
            },
            body_ref: None,
        };
        self.metadata.register_function(name, Arc::new(entry));
        let type_def = TypeDefinition {
            name: fn_type_name.clone(),
            kind: TypeDefinitionKind::Function {
                parameters,
                generic_parameters,
                return_type: return_type_name,
            },
            source_ref: SourceLocation(source_ref.0, source_ref.1),
            ast_ref: CheckerAstRef::ExternalFn {
                params,
                return_type,
                type_params,
                kind,
            },
        };
        self.metadata
            .register_type(fn_type_name, Arc::new(type_def));
    }

    pub(super) fn register_param_sigs(&mut self, module: &Module, file_id: FileId) {
        let Some(params) = module.definitions.iter().find_map(|def| {
            if let Definition::ModuleHeader { params, .. } = def {
                Some(params)
            } else {
                None
            }
        }) else {
            return;
        };

        let type_imports = Self::build_type_import_map(module, &self.metadata);
        for param in params {
            if param.path.len() < 2 {
                continue;
            }
            let concrete_module = param.path[0].clone();
            let sig_name = param.path.last().unwrap();

            let fn_data: Vec<(String, Vec<Parameter>, AstType)> = if let Some(sig_fns) = self
                .get_sig_functions(
                    sig_name,
                    &ModuleName::from_str(&concrete_module),
                    &type_imports,
                ) {
                sig_fns
                    .into_iter()
                    .map(|f| (f.name, f.parameters, f.return_type))
                    .collect()
            } else {
                self.metadata
                    .functions_in_module(&ModuleName::from_str(&concrete_module))
                    .into_iter()
                    .filter_map(|fdef| match &fdef.ast_ref {
                        CheckerAstRef::Function(func, _) => Some((
                            fdef.name.name.clone(),
                            func.parameters
                                .iter()
                                .map(|p| crate::ast::Parameter {
                                    name: p.name.clone(),
                                    param_type: self.resolve_type(
                                        &p.param_type,
                                        &ModuleName::from_str(&concrete_module),
                                        &type_imports,
                                    ),
                                    span: p.span,
                                })
                                .collect(),
                            self.resolve_type(
                                &func.return_type,
                                &ModuleName::from_str(&concrete_module),
                                &type_imports,
                            ),
                        )),
                        CheckerAstRef::ExternalFn {
                            params,
                            return_type,
                            ..
                        } => Some((fdef.name.name.clone(), params.clone(), return_type.clone())),
                        _ => None,
                    })
                    .collect()
            };

            for (fn_name, fn_params, ret_type) in fn_data {
                let fn_name_key = FunctionName {
                    name: fn_name.to_string(),
                    module: ModuleName::from_str(&param.name),
                    kind: FunctionNameKind::Function,
                };
                self.insert_fn(
                    fn_name_key,
                    fn_params,
                    ret_type,
                    vec![],
                    FunctionKind::External,
                    Visibility::Public,
                    SourceLocation(file_id, Span::dummy()),
                );
            }
            let sig_module = param.path[0].clone();
            let sig_name = param.path.last().unwrap().clone();
            let key = ImplKey {
                type_name: TypeName {
                    name: param.name.clone(),
                    module: ModuleName::from_str("__param__"),
                },
                trait_name: TraitName {
                    name: sig_name,
                    module: ModuleName::from_str(&sig_module),
                },
            };
            self.param_bindings.entry(key.clone()).or_insert_with(|| {
                Arc::new(ImplDefinition {
                    key,
                    module: ModuleName::from_str(&sig_module),
                    source_ref: SourceLocation(file_id, Span::dummy()),
                    ast_ref: NoAst,
                })
            });
        }
    }

    #[allow(deprecated)]
    pub(super) fn collect_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
        module_name: &str,
    ) -> Result<(), TypeError> {
        let type_imports = Self::build_type_import_map(module, &self.metadata);
        for definition in &module.definitions {
            if let Definition::Struct(struct_def) = definition {
                let type_name = TypeName {
                    name: struct_def.name.clone(),
                    module: ModuleName::from_str(module_name),
                };
                let entry = TypeDefinition {
                    name: type_name.clone(),
                    kind: TypeDefinitionKind::Struct {
                        fields: struct_def
                            .fields
                            .iter()
                            .map(|f| FieldDefinition {
                                name: f.name.clone(),
                                type_name: ast_type_to_type_name(&f.field_type, module_name),
                            })
                            .collect(),
                    },
                    source_ref: SourceLocation(file_id, struct_def.span),
                    ast_ref: CheckerAstRef::Struct(Arc::clone(struct_def)),
                };
                for f in &struct_def.fields {
                    let resolved = self.resolve_type(
                        &f.field_type,
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    );
                    self.validate_type_with_params(
                        &resolved,
                        f.span,
                        file_id,
                        &[],
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    )?;
                }
                self.metadata.register_type(type_name, Arc::new(entry));
            }
        }

        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    let resolved_return = self.resolve_type(
                        &func.return_type,
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    );
                    self.validate_type_with_params(
                        &resolved_return,
                        func.span,
                        file_id,
                        &func.type_params,
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    )?;
                    for param in &func.parameters {
                        let resolved_param_type = self.resolve_type(
                            &param.param_type,
                            &ModuleName::from_str(module_name),
                            &type_imports,
                        );
                        self.validate_type_with_params(
                            &resolved_param_type,
                            param.span,
                            file_id,
                            &func.type_params,
                            &ModuleName::from_str(module_name),
                            &type_imports,
                        )?;
                    }
                    let fn_key = FunctionName {
                        name: func.name.to_string(),
                        module: ModuleName::from_str(module_name),
                        kind: FunctionNameKind::Function,
                    };
                    let fn_type_name = TypeName {
                        name: fn_key.name.clone(),
                        module: fn_key.module.clone(),
                    };
                    let entry = FunctionDefinition {
                        name: fn_key.clone(),
                        visibility: if func.is_pub {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        },
                        type_name: fn_type_name.clone(),
                        source_ref: SourceLocation(file_id, func.span),
                        ast_ref: CheckerAstRef::Function(Arc::clone(func), FunctionKind::Bytecode),
                        body_ref: None,
                    };
                    self.metadata.register_function(fn_key, Arc::new(entry));
                    let fn_parameters: Vec<ParameterDefinition> = func
                        .parameters
                        .iter()
                        .map(|p| ParameterDefinition {
                            name: p.name.clone(),
                            type_name: ast_type_to_type_name(
                                &self.resolve_type(
                                    &p.param_type,
                                    &ModuleName::from_str(module_name),
                                    &type_imports,
                                ),
                                module_name,
                            ),
                        })
                        .collect();
                    let fn_generic_parameters: Vec<GenericParameterDefinition> = func
                        .type_params
                        .iter()
                        .map(|tp| GenericParameterDefinition {
                            name: tp.name.clone(),
                            constraints: tp
                                .bounds
                                .iter()
                                .map(|b| TraitName {
                                    name: b.clone(),
                                    module: fn_type_name.module.clone(),
                                })
                                .collect(),
                        })
                        .collect();
                    let fn_type_def = TypeDefinition {
                        name: fn_type_name.clone(),
                        kind: TypeDefinitionKind::Function {
                            parameters: fn_parameters,
                            generic_parameters: fn_generic_parameters,
                            return_type: ast_type_to_type_name(&resolved_return, module_name),
                        },
                        source_ref: SourceLocation(file_id, func.span),
                        ast_ref: CheckerAstRef::Function(Arc::clone(func), FunctionKind::Bytecode),
                    };
                    self.metadata
                        .register_type(fn_type_name, Arc::new(fn_type_def));
                }
                Definition::ExternalFunction(ext_func) => {
                    self.validate_type_with_params(
                        &ext_func.return_type,
                        ext_func.span,
                        file_id,
                        &ext_func.type_params,
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    )?;
                    for param in &ext_func.parameters {
                        self.validate_type_with_params(
                            &param.param_type,
                            param.span,
                            file_id,
                            &ext_func.type_params,
                            &ModuleName::from_str(module_name),
                            &type_imports,
                        )?;
                    }
                    let resolved_params: Vec<_> = ext_func
                        .parameters
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: self.resolve_type(
                                &p.param_type,
                                &ModuleName::from_str(module_name),
                                &type_imports,
                            ),
                            span: p.span,
                        })
                        .collect();
                    let fn_key = match ext_func.name.rsplit_once("::") {
                        Some((module, name)) => FunctionName {
                            name: name.to_string(),
                            module: ModuleName::from_str(module),
                            kind: FunctionNameKind::Function,
                        },
                        None => FunctionName {
                            name: ext_func.name.to_string(),
                            module: ModuleName::unqualified(),
                            kind: FunctionNameKind::Function,
                        },
                    };
                    let resolved_return = self.resolve_type(
                        &ext_func.return_type,
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    );
                    self.insert_fn(
                        fn_key,
                        resolved_params,
                        resolved_return,
                        ext_func.type_params.clone(),
                        FunctionKind::External,
                        if ext_func.is_pub {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        },
                        SourceLocation(file_id, ext_func.span),
                    );
                }
                Definition::Signature(s) => {
                    let type_name = TypeName {
                        name: s.name.clone(),
                        module: ModuleName::from_str(module_name),
                    };
                    let entry = TypeDefinition {
                        name: type_name.clone(),
                        kind: TypeDefinitionKind::Signature {
                            entries: s
                                .functions
                                .iter()
                                .map(|f| SignatureEntry {
                                    name: f.name.clone(),
                                    type_name: ast_type_to_type_name(&f.return_type, module_name),
                                })
                                .collect(),
                        },
                        source_ref: SourceLocation(file_id, s.span),
                        ast_ref: CheckerAstRef::Signature(Arc::clone(s)),
                    };
                    self.metadata.register_type(type_name, Arc::new(entry));
                }
                Definition::ModuleBinding {
                    name,
                    sig_path,
                    impl_path,
                    span,
                } => {
                    if sig_path.len() >= 2 && !impl_path.is_empty() {
                        let sig_module = sig_path[0].clone();
                        let sig_name = sig_path.last().unwrap().clone();
                        let concrete = impl_path[0].clone();
                        let key = ImplKey {
                            type_name: TypeName {
                                name: name.clone(),
                                module: ModuleName::from_str("__param__"),
                            },
                            trait_name: TraitName {
                                name: sig_name,
                                module: ModuleName::from_str(&sig_module),
                            },
                        };
                        let entry = ImplDefinition {
                            key: key.clone(),
                            module: ModuleName::from_str(&concrete),
                            source_ref: SourceLocation(file_id, *span),
                            ast_ref: NoAst,
                        };
                        self.param_bindings.insert(key, Arc::new(entry));
                    }
                }
                Definition::Struct(_)
                | Definition::Use { .. }
                | Definition::ModuleHeader { .. }
                | Definition::WiringSite { .. } => {}
                Definition::Trait(s) => {
                    let trait_name = TraitName {
                        name: s.name.clone(),
                        module: ModuleName::from_str(module_name),
                    };
                    let entry = TraitDefinition {
                        name: trait_name.clone(),
                        functions: s
                            .functions
                            .iter()
                            .map(|f| SignatureEntry {
                                name: f.name.clone(),
                                type_name: ast_type_to_type_name(&f.return_type, module_name),
                            })
                            .collect(),
                        witness_ref: NoWitness,
                        source_ref: SourceLocation(file_id, s.span),
                        ast_ref: CheckerAstRef::Trait(Arc::clone(s)),
                    };
                    self.metadata.traits.insert(trait_name, Arc::new(entry));
                }
                Definition::TraitImpl(impl_arc) => {
                    let type_name = &impl_arc.type_name;
                    let trait_name = &impl_arc.trait_name;
                    let functions = &impl_arc.functions;
                    let span = &impl_arc.span;
                    let trait_fns = self.get_trait_functions(
                        trait_name,
                        &ModuleName::from_str(module_name),
                        &type_imports,
                    );
                    if let Some(trait_fns) = trait_fns {
                        for trait_fn in &trait_fns {
                            let expected_name = &trait_fn.name;
                            let has_fn = functions.iter().any(|f| &f.name == expected_name);
                            if !has_fn {
                                return Err(TypeError::TraitImplMissingFunction {
                                    type_name: type_name.clone(),
                                    trait_name: trait_name.clone(),
                                    function_name: expected_name.clone(),
                                    span: *span,
                                    file_id,
                                });
                            }
                        }
                    } else {
                        return Err(TypeError::UnknownTrait {
                            name: trait_name.clone(),
                            span: *span,
                            file_id,
                        });
                    }
                    let sym_type_name = TypeName {
                        name: type_name.clone(),
                        module: ModuleName::from_str(module_name),
                    };
                    let sym_trait_name = TraitName {
                        name: trait_name.clone(),
                        module: ModuleName::from_str(module_name),
                    };
                    let key = ImplKey {
                        type_name: sym_type_name.clone(),
                        trait_name: sym_trait_name.clone(),
                    };
                    let impl_entry = ImplDefinition {
                        key: key.clone(),
                        module: ModuleName::from_str(module_name),
                        source_ref: SourceLocation(file_id, *span),
                        ast_ref: CheckerAstRef::Impl(Arc::clone(impl_arc)),
                    };
                    self.metadata.impls.insert(key, Arc::new(impl_entry));
                    for func in functions {
                        let resolved_return = Self::substitute_self(
                            &self.resolve_type(
                                &func.return_type,
                                &ModuleName::from_str(module_name),
                                &type_imports,
                            ),
                            type_name,
                        );
                        let impl_fn_key = {
                            let mn = ModuleName::from_str(module_name);
                            FunctionName {
                                name: func.name.to_string(),
                                module: mn.clone(),
                                kind: FunctionNameKind::Impl {
                                    type_name: TypeName {
                                        name: type_name.to_string(),
                                        module: mn.clone(),
                                    },
                                    trait_name: TraitName {
                                        name: trait_name.to_string(),
                                        module: mn,
                                    },
                                },
                            }
                        };
                        let entry = FunctionDefinition {
                            name: impl_fn_key.clone(),
                            visibility: Visibility::Private,
                            type_name: ast_type_to_type_name(&resolved_return, module_name),
                            source_ref: SourceLocation(file_id, func.span),
                            ast_ref: CheckerAstRef::ImplFunction(
                                Arc::clone(func),
                                type_name.to_string(),
                                FunctionKind::Bytecode,
                            ),
                            body_ref: None,
                        };
                        self.metadata
                            .register_function(impl_fn_key, Arc::new(entry));
                    }
                }
            }
        }
        Ok(())
    }
}
