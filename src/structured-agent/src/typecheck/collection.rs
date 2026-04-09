use super::refs::{CheckerAstRef, CheckerRefs, FunctionKind, NoWitness, SourceLocation};
use super::{TypeChecker, ast_type_to_type_name};
use crate::ast::{Definition, Module, Parameter, Type as AstType, TypeParam};

use crate::types::{FileId, Span};
use nonempty::NonEmpty;
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
                module: ModuleName::new(NonEmpty::new(module.to_string())),
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

    pub(super) fn register_native_modules(
        &mut self,
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) {
        use super::refs::CheckerAstRef;
        use structured_agent_runtime::symbols::{ExportedName, ModuleDefinition, UseImport};
        for (mod_name, native_mod) in native_modules {
            let module_name = ModuleName::new(NonEmpty::new(mod_name.clone()));
            for func in native_mod.functions() {
                let fn_key = FunctionName {
                    name: func.name().to_string(),
                    module: module_name.clone(),
                    kind: FunctionNameKind::Function,
                };
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
                self.insert_fn(
                    fn_key,
                    parameters,
                    return_type,
                    type_params,
                    FunctionKind::External,
                    Visibility::Public,
                    SourceLocation(0, Span::dummy()),
                );
            }
            let exports = self
                .metadata
                .functions_in_module(&module_name)
                .into_iter()
                .map(|f| ExportedName::Function(f.name.clone()))
                .collect();
            let module_def = ModuleDefinition {
                name: module_name.clone(),
                visibility: Visibility::Public,
                exports,
                source_ref: SourceLocation(0, Span::dummy()),
                ast_ref: CheckerAstRef::Module(Arc::new(Module {
                    definitions: vec![],
                    span: Span::dummy(),
                    file_id: 0,
                })),
                use_imports: vec![],
            };
            self.metadata
                .modules
                .insert(module_name, Arc::new(module_def));
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
        let return_type_name = ast_type_to_type_name(&return_type, &name.module);
        let parameters: Vec<ParameterDefinition<CheckerRefs>> = params
            .iter()
            .map(|p| ParameterDefinition {
                name: p.name.clone(),
                type_name: p.param_type.clone(),
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
                return_type: return_type.clone(),
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

    pub(super) fn register_type_definitions(
        &mut self,
        module: &Module,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        for definition in &module.definitions {
            if let Definition::Struct(struct_def) = definition {
                let type_name = TypeName {
                    name: struct_def.name.clone(),
                    module: module_name.clone(),
                };
                let entry = TypeDefinition {
                    name: type_name.clone(),
                    kind: TypeDefinitionKind::Struct {
                        fields: struct_def
                            .fields
                            .iter()
                            .map(|f| FieldDefinition {
                                name: f.name.clone(),
                                type_name: f.field_type.clone(),
                            })
                            .collect(),
                    },
                    source_ref: SourceLocation(file_id, struct_def.span),
                    ast_ref: CheckerAstRef::Struct(Arc::clone(struct_def)),
                };

                self.metadata.register_type(type_name, Arc::new(entry));
            }
        }
    }

    pub(super) fn register_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    let fn_key = FunctionName {
                        name: func.name.to_string(),
                        module: module_name.clone(),
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
                    let fn_parameters: Vec<ParameterDefinition<CheckerRefs>> = func
                        .parameters
                        .iter()
                        .map(|p| ParameterDefinition {
                            name: p.name.clone(),
                            type_name: p.param_type.clone(),
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
                            return_type: func.return_type.clone(),
                        },
                        source_ref: SourceLocation(file_id, func.span),
                        ast_ref: CheckerAstRef::Function(Arc::clone(func), FunctionKind::Bytecode),
                    };
                    self.metadata
                        .register_type(fn_type_name, Arc::new(fn_type_def));
                }
                Definition::ExternalFunction(ext_func) => {
                    let fn_key = match ext_func.name.rsplit_once("::") {
                        Some((module, name)) => FunctionName {
                            name: name.to_string(),
                            module: ModuleName::new(
                                NonEmpty::from_vec(
                                    module.split("::").map(|s| s.to_string()).collect(),
                                )
                                .unwrap(),
                            ),
                            kind: FunctionNameKind::Function,
                        },
                        None => FunctionName {
                            name: ext_func.name.to_string(),
                            module: ModuleName::new(NonEmpty::new(String::new())),
                            kind: FunctionNameKind::Function,
                        },
                    };
                    self.insert_fn(
                        fn_key,
                        ext_func.parameters.iter().cloned().collect(),
                        ext_func.return_type.clone(),
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
                Definition::Struct(_)
                | Definition::Use { .. }
                | Definition::ModuleBinding { .. }
                | Definition::WiringSite { .. }
                | Definition::ModuleHeader { .. }
                | Definition::Signature(_) => {}
                Definition::Trait(s) => {
                    let trait_name = TraitName {
                        name: s.name.clone(),
                        module: module_name.clone(),
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

                    let sym_type_name = TypeName {
                        name: type_name.clone(),
                        module: module_name.clone(),
                    };
                    let sym_trait_name = TraitName {
                        name: trait_name.clone(),
                        module: module_name.clone(),
                    };
                    let key = ImplKey {
                        type_name: sym_type_name.clone(),
                        trait_name: sym_trait_name.clone(),
                    };
                    let impl_entry = ImplDefinition {
                        key: key.clone(),
                        module: module_name.clone(),
                        source_ref: SourceLocation(file_id, *span),
                        ast_ref: CheckerAstRef::Impl(Arc::clone(impl_arc)),
                    };
                    self.metadata.impls.insert(key, Arc::new(impl_entry));
                    for func in functions {
                        let resolved_return = Self::substitute_self(&func.return_type, type_name);
                        let impl_fn_key = {
                            let mn = module_name.clone();
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
    }
}
