use super::db::{ArcPtr, SymbolTablesInput, TypeCheckDb};
use super::refs::{CheckerAstRef, CheckerRefs, FunctionKind, NoWitness, SourceLocation};
use crate::ast::{Definition, Module, Parameter, ParsedModule, Type as AstType, TypeParam};
use crate::types::{FileId, Span};
use nonempty::NonEmpty;
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    ExportedName, FieldDefinition, FunctionDefinition, FunctionName, GenericParameterDefinition,
    ImplDefinition, ImplKey, MetaData, ModuleDefinition, ModuleName, ParameterDefinition,
    SignatureEntry, SymbolQuery, TypeDefinition, TypeDefinitionKind, TypeName, Visibility,
};
use structured_agent_runtime::types::Module as RuntimeModule;

pub(super) struct SymbolTableBuilder {
    metadata: MetaData<CheckerRefs>,
}

impl SymbolTableBuilder {
    pub(super) fn new() -> Self {
        let mut builder = Self {
            metadata: MetaData::default(),
        };
        builder.seed_builtin_types();
        builder
    }

    fn seed_builtin_types(&mut self) {
        let primitives = [
            ("Unit", "prelude"),
            ("Boolean", "prelude"),
            ("String", "prelude"),
            ("Int", "prelude"),
        ];
        for (name, module) in primitives {
            let type_name = TypeName::new(ModuleName::new(NonEmpty::new(module.to_string())), name);
            let entry = TypeDefinition {
                name: type_name.clone(),
                kind: TypeDefinitionKind::Primitive,
                source_ref: SourceLocation(0, crate::types::Span::dummy()),
                ast_ref: CheckerAstRef::Primitive,
            };
            self.metadata.register_type(type_name, Arc::new(entry));
        }

        let t_param = GenericParameterDefinition {
            name: "T".to_string(),
            constraints: vec![],
        };

        let list_name = TypeName::new(
            ModuleName::new(NonEmpty::new("prelude".to_string())),
            "List",
        );
        self.metadata.register_type(
            list_name.clone(),
            Arc::new(TypeDefinition {
                name: list_name,
                kind: TypeDefinitionKind::Native {
                    generic_parameters: vec![t_param.clone()],
                    factory: Arc::new(structured_agent_runtime::runtime_value::ListValueFactory),
                },
                source_ref: SourceLocation(0, crate::types::Span::dummy()),
                ast_ref: CheckerAstRef::Primitive,
            }),
        );

        let option_name = TypeName::new(
            ModuleName::new(NonEmpty::new("prelude".to_string())),
            "Option",
        );
        self.metadata.register_type(
            option_name.clone(),
            Arc::new(TypeDefinition {
                name: option_name,
                kind: TypeDefinitionKind::Native {
                    generic_parameters: vec![t_param],
                    factory: Arc::new(structured_agent_runtime::runtime_value::OptionValueFactory),
                },
                source_ref: SourceLocation(0, crate::types::Span::dummy()),
                ast_ref: CheckerAstRef::Primitive,
            }),
        );
    }

    fn register_native_modules(
        &mut self,
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) {
        for (mod_name, native_mod) in native_modules {
            let module_name = ModuleName::new(NonEmpty::new(mod_name.clone()));
            for func in native_mod.functions() {
                let fn_key = FunctionName::new(module_name.clone(), func.name());
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
                is_entry: false,
                exports,
                source_ref: SourceLocation(0, Span::dummy()),
                ast_ref: CheckerAstRef::Module(Arc::new(Module {
                    definitions: vec![],
                    span: Span::dummy(),
                    file_id: 0,
                })),
                parent_module: None,
            };
            self.metadata
                .modules
                .insert(module_name, Arc::new(module_def));
        }
    }

    fn runtime_type_to_ast(ty: &structured_agent_runtime::types::Type) -> AstType {
        use structured_agent_runtime::types::Type as RT;
        match ty {
            RT::Parameterized(type_name, args) => AstType {
                name: type_name.name().to_string(),
                args: args.iter().map(|a| Self::runtime_type_to_ast(a)).collect(),
            },
            RT::Struct(tn) => AstType::simple(tn.name().to_string()),
            RT::Generic(name) => AstType::simple(name.clone()),
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
        let fn_type_name = TypeName::new(name.module(), name.name());
        let parameters: Vec<ParameterDefinition<CheckerRefs>> = params
            .iter()
            .map(|p| ParameterDefinition {
                name: p.name.clone(),
                type_name: p.param_type.clone(),
                source_ref: SourceLocation(source_ref.0, p.span),
            })
            .collect();
        let generic_parameters: Vec<GenericParameterDefinition<CheckerRefs>> = type_params
            .iter()
            .map(|tp| GenericParameterDefinition {
                name: tp.name.clone(),
                constraints: tp.bounds.clone(),
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

    fn register_type_definitions(
        &mut self,
        module: &Module,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        for definition in &module.definitions {
            match definition {
                Definition::Signature(sig) => {
                    self.register_signature(sig, file_id, module_name);
                }
                Definition::Struct(struct_def) => {
                    self.register_struct(struct_def, file_id, module_name);
                }
                _ => {}
            }
        }
    }

    fn register_signature(
        &mut self,
        sig: &Arc<crate::ast::AstSignature>,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        let type_name = TypeName::new(module_name.clone(), sig.name.clone());
        let entry = TypeDefinition {
            name: type_name.clone(),
            kind: TypeDefinitionKind::Signature {
                entries: sig
                    .functions
                    .iter()
                    .map(|f| SignatureEntry {
                        name: f.name.clone(),
                        type_name: AstType::simple(&f.name),
                    })
                    .collect(),
            },
            source_ref: SourceLocation(file_id, sig.span),
            ast_ref: CheckerAstRef::Signature(Arc::clone(sig)),
        };
        self.metadata.register_type(type_name, Arc::new(entry));
    }

    fn register_struct(
        &mut self,
        struct_def: &Arc<crate::ast::StructDefinition>,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        let type_name = TypeName::new(module_name.clone(), struct_def.name.clone());
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
                generic_parameters: struct_def
                    .type_params
                    .iter()
                    .map(|tp| GenericParameterDefinition {
                        name: tp.name.clone(),
                        constraints: tp.bounds.clone(),
                    })
                    .collect(),
            },
            source_ref: SourceLocation(file_id, struct_def.span),
            ast_ref: CheckerAstRef::Struct(Arc::clone(struct_def)),
        };
        self.metadata.register_type(type_name, Arc::new(entry));
    }

    fn register_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        let mut impl_counter: u32 = 0;
        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    self.register_regular_function(func, file_id, module_name);
                }
                Definition::ExternalFunction(ext_func) => {
                    self.register_external_function(ext_func, file_id, module_name);
                }
                Definition::Struct(_)
                | Definition::Use(_)
                | Definition::ModuleHeader { .. }
                | Definition::Signature(_)
                | Definition::InlineModule { .. } => {}
                Definition::Trait(s) => {
                    self.register_trait(s, file_id, module_name);
                }
                Definition::TraitImpl(impl_arc) => {
                    self.register_trait_impl(impl_arc, file_id, module_name, impl_counter);
                    impl_counter += 1;
                }
            }
        }
    }

    fn register_regular_function(
        &mut self,
        func: &Arc<crate::ast::Function>,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        let fn_key = FunctionName::new(module_name.clone(), func.name.to_string());
        let fn_type_name = TypeName::new(fn_key.module(), fn_key.name());
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
                source_ref: SourceLocation(file_id, p.span),
            })
            .collect();
        let fn_generic_parameters: Vec<GenericParameterDefinition<CheckerRefs>> = func
            .type_params
            .iter()
            .map(|tp| GenericParameterDefinition {
                name: tp.name.clone(),
                constraints: tp.bounds.clone(),
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

    fn register_external_function(
        &mut self,
        ext_func: &crate::ast::ExternalFunction,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        let fn_key = FunctionName::new(module_name.clone(), ext_func.name.to_string());
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

    fn register_trait(
        &mut self,
        trait_def: &Arc<crate::ast::AstTrait>,
        file_id: FileId,
        module_name: &ModuleName,
    ) {
        let type_name = TypeName::new(module_name.clone(), trait_def.name.clone());
        let entry = TypeDefinition {
            name: type_name.clone(),
            kind: TypeDefinitionKind::Trait {
                functions: trait_def
                    .functions
                    .iter()
                    .map(|f| SignatureEntry {
                        name: f.name.clone(),
                        type_name: AstType::simple(&f.name),
                    })
                    .collect(),
                witness_ref: NoWitness,
            },
            source_ref: SourceLocation(file_id, trait_def.span),
            ast_ref: CheckerAstRef::Trait(Arc::clone(trait_def)),
        };
        self.metadata.register_type(type_name, Arc::new(entry));
    }

    fn register_trait_impl(
        &mut self,
        impl_arc: &Arc<crate::ast::AstTraitImpl>,
        file_id: FileId,
        module_name: &ModuleName,
        discriminator: u32,
    ) {
        let type_name = &impl_arc.type_name;
        let trait_name = &impl_arc.trait_name;
        let functions = &impl_arc.functions;
        let span = &impl_arc.span;

        let key = ImplKey::new(module_name.clone(), Some(discriminator));
        let impl_entry = ImplDefinition {
            key: key.clone(),
            module: module_name.clone(),
            type_name: AstType::simple(type_name.clone()),
            trait_name: AstType::simple(trait_name.clone()),
            source_ref: SourceLocation(file_id, *span),
            ast_ref: CheckerAstRef::Impl(Arc::clone(impl_arc)),
        };
        self.metadata
            .impls
            .insert(key.clone(), Arc::new(impl_entry));

        for func in functions {
            self.register_impl_function(func, file_id, &key, type_name);
        }
    }

    fn register_impl_function(
        &mut self,
        func: &Arc<crate::ast::Function>,
        file_id: FileId,
        impl_key: &ImplKey,
        type_name: &str,
    ) {
        let impl_fn_key = FunctionName::for_impl(impl_key, func.name.to_string());
        let fn_type_name = TypeName::new(impl_fn_key.module(), impl_fn_key.name());
        let entry = FunctionDefinition {
            name: impl_fn_key.clone(),
            visibility: Visibility::Private,
            type_name: fn_type_name.clone(),
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

        let fn_parameters: Vec<ParameterDefinition<CheckerRefs>> = func
            .parameters
            .iter()
            .map(|p| ParameterDefinition {
                name: p.name.clone(),
                type_name: p.param_type.clone(),
                source_ref: SourceLocation(file_id, p.span),
            })
            .collect();
        let fn_generic_parameters: Vec<GenericParameterDefinition<CheckerRefs>> = func
            .type_params
            .iter()
            .map(|tp| GenericParameterDefinition {
                name: tp.name.clone(),
                constraints: tp.bounds.clone(),
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
            ast_ref: CheckerAstRef::ImplFunction(
                Arc::clone(func),
                type_name.to_string(),
                FunctionKind::Bytecode,
            ),
        };
        self.metadata
            .register_type(fn_type_name, Arc::new(fn_type_def));
    }

    pub(super) fn build_symbol_tables(
        mut self,
        db: &TypeCheckDb,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) -> SymbolTablesInput {
        self.register_native_modules(native_modules);
        for parsed in modules {
            let module_name = ModuleName::new(parsed.name.clone());
            self.register_type_definitions(&parsed.module, parsed.file_id, &module_name);
            self.register_function_signatures(&parsed.module, parsed.file_id, &module_name);
            self.register_module_definition(parsed, module_name);
        }
        let tables = SymbolTablesInput::new(
            db,
            ArcPtr::new(self.metadata.functions.clone()),
            ArcPtr::new(self.metadata.types.clone()),
            ArcPtr::new(self.metadata.impls.clone()),
            ArcPtr::new(self.metadata.modules.clone()),
        );
        tables
    }

    fn register_module_definition(
        &mut self,
        parsed: &ParsedModule,
        effective_module_name: ModuleName,
    ) {
        let exports = self.collect_module_exports(&effective_module_name);
        let parent_module = if parsed.is_inline {
            let mut segs: Vec<String> = parsed.name.iter().cloned().collect();
            segs.pop();
            NonEmpty::from_vec(segs).map(ModuleName::new)
        } else {
            None
        };
        let module_def = ModuleDefinition {
            name: effective_module_name.clone(),
            visibility: if parsed.is_entry {
                Visibility::Public
            } else {
                Visibility::Private
            },
            is_entry: parsed.is_entry,
            exports,
            source_ref: SourceLocation(parsed.file_id, crate::types::Span::dummy()),
            ast_ref: CheckerAstRef::Module(Arc::new(parsed.module.clone())),
            parent_module,
        };
        self.metadata
            .modules
            .insert(effective_module_name, Arc::new(module_def));

        self.register_module_as_type(parsed);
    }

    fn register_module_as_type(&mut self, parsed: &ParsedModule) {
        let module_name = ModuleName::new(parsed.name.clone());
        let type_name = TypeName::from(module_name);
        let entries: Vec<SignatureEntry<CheckerRefs>> = parsed
            .module
            .definitions
            .iter()
            .filter_map(|def| match def {
                Definition::Function(func) if func.is_pub => Some(SignatureEntry {
                    name: func.name.to_string(),
                    type_name: AstType::simple(&func.name),
                }),
                _ => None,
            })
            .collect();
        let module_type = TypeDefinition {
            name: type_name.clone(),
            kind: TypeDefinitionKind::Signature { entries },
            source_ref: SourceLocation(parsed.file_id, crate::types::Span::dummy()),
            ast_ref: CheckerAstRef::Module(Arc::new(parsed.module.clone())),
        };
        self.metadata
            .register_type(type_name, Arc::new(module_type));
    }

    fn collect_module_exports(&self, module_name: &ModuleName) -> Vec<ExportedName> {
        let mut exports: Vec<ExportedName> = self
            .metadata
            .functions_in_module(module_name)
            .into_iter()
            .filter(|f| matches!(f.visibility, Visibility::Public))
            .map(|f| ExportedName::Function(f.name.clone()))
            .collect();
        let type_exports: Vec<ExportedName> = self
            .metadata
            .types
            .values()
            .filter(|t| &t.name.module() == module_name)
            .filter_map(|t| match &t.kind {
                TypeDefinitionKind::Struct { .. } => Some(ExportedName::Type(t.name.clone())),
                TypeDefinitionKind::Trait { .. } => Some(ExportedName::Trait(t.name.clone())),
                TypeDefinitionKind::Signature { .. } => Some(ExportedName::Type(t.name.clone())),
                _ => None,
            })
            .collect();
        exports.extend(type_exports);
        exports
    }
}
