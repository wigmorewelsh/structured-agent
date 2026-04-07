use crate::ast::{
    Definition, Expression, Function, Module, ModuleParam, Parameter, ParsedModule, SelectClause,
    SigFunction, Statement, Type as AstType, TypeParam,
};
use crate::typecheck::error::TypeError;
use crate::typed_ast;
use crate::types::{FileId, Span, Spanned};
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    AstRef, BodyRef, ExportedName, FieldDefinition, FunctionDefinition, FunctionName,
    FunctionNameKind, GenericParameterDefinition, ImplDefinition, ImplKey, MetaData,
    ModuleDefinition, ModuleName, ParameterDefinition, References, SignatureEntry, SourceRef,
    SymbolQuery, TraitDefinition, TraitName, TypeDefinition, TypeDefinitionKind, TypeName,
    Visibility, WitnessRef,
};
use structured_agent_runtime::types::Module as RuntimeModule;

pub type ModuleVisibility = HashMap<String, bool>;
pub type AliasToQualified = HashMap<String, String>;

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionKind {
    Bytecode,
    External,
}

#[derive(Clone)]
pub struct SourceLocation(pub FileId, pub Span);
pub struct NoBody;
#[derive(Clone)]
pub struct NoWitness;

#[derive(Clone)]
pub enum CheckerAstRef {
    Function(Arc<crate::ast::Function>, FunctionKind),
    ImplFunction(Arc<crate::ast::Function>, String, FunctionKind),
    ExternalFn {
        params: Vec<crate::ast::Parameter>,
        return_type: AstType,
        type_params: Vec<TypeParam>,
        kind: FunctionKind,
    },
    Struct(Arc<crate::ast::StructDefinition>),
    Trait(Arc<crate::ast::AstTrait>),
    Impl(Arc<crate::ast::AstTraitImpl>),
    Module(Arc<crate::ast::Module>),
    Signature(Arc<crate::ast::AstSignature>),
    Builtin,
    ModuleParamBinding,
}

impl SourceRef for SourceLocation {}
impl AstRef for CheckerAstRef {}
impl BodyRef for NoBody {}
impl WitnessRef for NoWitness {}

pub struct CheckerRefs;

impl References for CheckerRefs {
    type Source = SourceLocation;
    type Ast = CheckerAstRef;
    type Body = NoBody;
    type Witness = NoWitness;
}

#[derive(Clone)]
pub enum TypedCheckerAstRef {
    Function(Arc<typed_ast::Function>, FunctionKind),
    ImplFunction(Arc<typed_ast::Function>, String, FunctionKind),
    Other(CheckerAstRef),
}

impl AstRef for TypedCheckerAstRef {}

pub struct TypedRefs;

impl References for TypedRefs {
    type Source = SourceLocation;
    type Ast = TypedCheckerAstRef;
    type Body = NoBody;
    type Witness = NoWitness;
}

fn ast_type_to_type_name(ty: &AstType, module_name: &str) -> TypeName {
    match ty {
        AstType::Struct(name) => TypeName {
            name: name.clone(),
            module: ModuleName::from_str(module_name),
        },
        AstType::List(_) => TypeName {
            name: "List".to_string(),
            module: ModuleName::from_str("prelude"),
        },
        AstType::Option(_) => TypeName {
            name: "Option".to_string(),
            module: ModuleName::from_str("prelude"),
        },
        other => TypeName {
            name: other.to_string(),
            module: ModuleName::from_str("prelude"),
        },
    }
}

pub struct TypeChecker {
    metadata: MetaData<CheckerRefs>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Parameter>,
    return_type: AstType,
    kind: FunctionKind,
    type_params: Vec<TypeParam>,
}

#[derive(Debug, Clone)]
struct TypeEnvironment {
    variables: HashMap<String, (AstType, Span)>,
    parent: Option<Box<TypeEnvironment>>,
}

struct CheckContext<'a> {
    file_id: FileId,
    alias_map: &'a HashMap<String, String>,
    alias_to_qualified: &'a AliasToQualified,
    module_name: Option<&'a str>,
    module_params: &'a [ModuleParam],
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            metadata: MetaData::default(),
        };
        checker.seed_builtin_types();
        checker
    }

    fn seed_builtin_types(&mut self) {
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
                ast_ref: CheckerAstRef::Builtin,
            };
            self.metadata.register_type(type_name, Arc::new(entry));
        }
    }

    pub fn check_modules(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) -> Result<(MetaData<TypedRefs>, HashMap<String, typed_ast::Module>), TypeError> {
        for parsed in modules {
            let effective_name = if parsed.is_entry {
                "main"
            } else {
                parsed.name.as_str()
            };
            self.collect_native_sigs(parsed, native_modules);
            self.collect_function_signatures(&parsed.module, parsed.file_id, effective_name)?;
            let exports: Vec<ExportedName> = self
                .metadata
                .functions_in_module(&ModuleName::from_str(effective_name))
                .into_iter()
                .filter(|f| matches!(f.visibility, Visibility::Public))
                .map(|f| ExportedName::Function(f.name.clone()))
                .collect();
            let use_aliases: Vec<(String, String)> = parsed
                .module
                .definitions
                .iter()
                .filter_map(|def| {
                    if let Definition::Use { path, alias, .. } = def
                        && path.len() >= 2
                    {
                        let qualified = format!("{}::{}", path[0], path.last().unwrap());
                        let local = alias
                            .clone()
                            .unwrap_or_else(|| path.last().unwrap().clone());
                        Some((local, qualified))
                    } else {
                        None
                    }
                })
                .collect();
            let module_def = ModuleDefinition {
                name: ModuleName::from_str(effective_name),
                visibility: if parsed.is_entry {
                    Visibility::Public
                } else {
                    Visibility::Private
                },
                exports,
                source_ref: SourceLocation(parsed.file_id, crate::types::Span::dummy()),
                ast_ref: CheckerAstRef::Module(Arc::new(parsed.module.clone())),
                use_aliases,
            };
            self.metadata
                .modules
                .insert(ModuleName::from_str(effective_name), Arc::new(module_def));
        }
        for parsed in modules {
            self.register_param_sigs(&parsed.module, parsed.file_id);
        }
        let mut typed_modules = HashMap::new();
        for parsed in modules {
            typed_modules.insert(
                parsed.name.clone(),
                self.check_single_module_expressions(parsed)?,
            );
        }
        let effective_name_to_typed: HashMap<String, &typed_ast::Module> = modules
            .iter()
            .map(|p| {
                let eff = if p.is_entry {
                    "main".to_string()
                } else {
                    p.name.clone()
                };
                (eff, typed_modules.get(&p.name).unwrap())
            })
            .collect();
        let mut typed_metadata: MetaData<TypedRefs> = MetaData::default();
        for fn_def in self.metadata.all_functions() {
            let typed_ast_ref = match &fn_def.ast_ref {
                CheckerAstRef::Function(_, kind) => {
                    let module_name = fn_def.name.module.to_string();
                    let typed_module = effective_name_to_typed[&module_name];
                    let typed_fn = typed_module
                        .definitions
                        .iter()
                        .find_map(|d| {
                            if let typed_ast::Definition::Function(f) = d {
                                if f.name == fn_def.name.name {
                                    Some(f.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .expect("typed function not found");
                    TypedCheckerAstRef::Function(Arc::new(typed_fn), kind.clone())
                }
                CheckerAstRef::ImplFunction(_, type_name_str, kind) => {
                    let module_name = fn_def.name.module.to_string();
                    let typed_module = effective_name_to_typed[&module_name];
                    let typed_fn = typed_module
                        .definitions
                        .iter()
                        .find_map(|d| {
                            if let typed_ast::Definition::TraitImpl {
                                type_name,
                                functions,
                                ..
                            } = d
                            {
                                if type_name == type_name_str {
                                    functions
                                        .iter()
                                        .find(|f| f.name == fn_def.name.name)
                                        .cloned()
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .expect("typed impl function not found");
                    TypedCheckerAstRef::ImplFunction(
                        Arc::new(typed_fn),
                        type_name_str.clone(),
                        kind.clone(),
                    )
                }
                other => TypedCheckerAstRef::Other(other.clone()),
            };
            let typed_fn_def = FunctionDefinition {
                name: fn_def.name.clone(),
                visibility: fn_def.visibility.clone(),
                type_name: fn_def.type_name.clone(),
                source_ref: SourceLocation(fn_def.source_ref.0, fn_def.source_ref.1),
                ast_ref: typed_ast_ref,
                body_ref: None,
            };
            typed_metadata
                .functions
                .insert(fn_def.name.clone(), Arc::new(typed_fn_def));
        }
        for type_def in self.metadata.all_types() {
            let new_def = TypeDefinition {
                name: type_def.name.clone(),
                kind: type_def.kind.clone(),
                source_ref: SourceLocation(type_def.source_ref.0, type_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::Other(type_def.ast_ref.clone()),
            };
            typed_metadata
                .types
                .insert(type_def.name.clone(), Arc::new(new_def));
        }
        for (trait_key, trait_def) in &self.metadata.traits {
            let new_def = TraitDefinition {
                name: trait_def.name.clone(),
                functions: trait_def.functions.clone(),
                witness_ref: NoWitness,
                source_ref: SourceLocation(trait_def.source_ref.0, trait_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::Other(trait_def.ast_ref.clone()),
            };
            typed_metadata
                .traits
                .insert(trait_key.clone(), Arc::new(new_def));
        }
        for (impl_key, impl_def) in &self.metadata.impls {
            let new_def = ImplDefinition {
                key: impl_def.key.clone(),
                module: impl_def.module.clone(),
                source_ref: SourceLocation(impl_def.source_ref.0, impl_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::Other(impl_def.ast_ref.clone()),
            };
            typed_metadata
                .impls
                .insert(impl_key.clone(), Arc::new(new_def));
        }
        for (module_key, module_def) in &self.metadata.modules {
            let new_def = ModuleDefinition {
                name: module_def.name.clone(),
                visibility: module_def.visibility.clone(),
                exports: module_def.exports.clone(),
                source_ref: SourceLocation(module_def.source_ref.0, module_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::Other(module_def.ast_ref.clone()),
                use_aliases: module_def.use_aliases.clone(),
            };
            typed_metadata
                .modules
                .insert(module_key.clone(), Arc::new(new_def));
        }
        Ok((typed_metadata, typed_modules))
    }

    #[allow(deprecated)]
    fn collect_native_sigs(
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

    fn check_single_module_expressions(
        &mut self,
        parsed: &ParsedModule,
    ) -> Result<typed_ast::Module, TypeError> {
        let effective_name = if parsed.is_entry {
            "main"
        } else {
            parsed.name.as_str()
        };
        let module = &parsed.module;
        let alias_map = Self::build_alias_map(module);
        let alias_to_qualified = self.build_alias_to_qualified(module);
        let module_params = module
            .definitions
            .iter()
            .find_map(|def| {
                if let Definition::ModuleHeader { params, .. } = def {
                    Some(params.as_slice())
                } else {
                    None
                }
            })
            .unwrap_or(&[]);
        let ctx = CheckContext {
            file_id: parsed.file_id,
            alias_map: &alias_map,
            alias_to_qualified: &alias_to_qualified,
            module_name: Some(effective_name),
            module_params,
        };
        let typed_definitions = module
            .definitions
            .iter()
            .map(|def| self.check_definition(def, &ctx))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(typed_ast::Module {
            definitions: typed_definitions,
            span: module.span,
            file_id: parsed.file_id,
        })
    }

    fn check_definition(
        &mut self,
        definition: &Definition,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Definition, TypeError> {
        match definition {
            Definition::Function(func) => Ok(typed_ast::Definition::Function(
                self.check_function(func, ctx)?,
            )),
            Definition::ExternalFunction(f) => {
                Ok(typed_ast::Definition::ExternalFunction((**f).clone()))
            }
            Definition::Struct(s) => Ok(typed_ast::Definition::Struct((**s).clone())),
            Definition::Use {
                path,
                alias,
                is_pub,
                span,
            } => Ok(typed_ast::Definition::Use {
                path: path.clone(),
                alias: alias.clone(),
                is_pub: *is_pub,
                span: *span,
            }),
            Definition::ModuleHeader { name, params, span } => {
                Ok(typed_ast::Definition::ModuleHeader {
                    name: name.clone(),
                    params: params.clone(),
                    span: *span,
                })
            }
            Definition::ModuleBinding {
                name,
                sig_path,
                impl_path,
                span,
            } => Ok(typed_ast::Definition::ModuleBinding {
                name: name.clone(),
                sig_path: sig_path.clone(),
                impl_path: impl_path.clone(),
                span: *span,
            }),
            Definition::WiringSite { name, args, span } => Ok(typed_ast::Definition::WiringSite {
                name: name.clone(),
                args: args.clone(),
                span: *span,
            }),
            Definition::Signature(s) => Ok(typed_ast::Definition::Signature {
                name: s.name.clone(),
                functions: s.functions.clone(),
                span: s.span,
            }),
            Definition::Trait(s) => Ok(typed_ast::Definition::Trait {
                name: s.name.clone(),
                functions: s.functions.clone(),
                span: s.span,
            }),
            Definition::TraitImpl(t) => {
                let (type_name, trait_name, functions, span) =
                    (&t.type_name, &t.trait_name, &t.functions, &t.span);
                let typed_functions: Result<Vec<typed_ast::Function>, TypeError> = functions
                    .iter()
                    .map(|func| {
                        let concrete = Self::substitute_self_in_fn(func, type_name);
                        self.check_function(&concrete, ctx)
                    })
                    .collect();
                Ok(typed_ast::Definition::TraitImpl {
                    type_name: type_name.clone(),
                    trait_name: trait_name.clone(),
                    functions: typed_functions?,
                    span: *span,
                })
            }
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

    fn get_function_sig(&self, name: &FunctionName) -> Option<FunctionSignature> {
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

    fn get_struct_fields(&self, name: &str) -> Option<Vec<(String, AstType)>> {
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

    fn get_trait_functions(&self, name: &str) -> Option<Vec<crate::ast::SigFunction>> {
        self.metadata.trait_by_name(name).and_then(|td| {
            if let CheckerAstRef::Trait(t) = &td.ast_ref {
                Some(t.functions.clone())
            } else {
                None
            }
        })
    }

    fn type_implements_trait(&self, type_name: &str, trait_name: &str) -> bool {
        self.metadata
            .impls
            .keys()
            .any(|k| k.type_name.name == type_name && k.trait_name.name == trait_name)
    }

    fn register_param_sigs(&mut self, module: &Module, file_id: FileId) {
        let Some(params) = module.definitions.iter().find_map(|def| {
            if let Definition::ModuleHeader { params, .. } = def {
                Some(params)
            } else {
                None
            }
        }) else {
            return;
        };

        for param in params {
            if param.path.len() < 2 {
                continue;
            }
            let concrete_module = param.path[0].clone();
            let sig_name = param.path.last().unwrap();

            let fn_data: Vec<(String, Vec<Parameter>, AstType)> = if let Some(sig_fns) =
                self.get_sig_functions(sig_name)
            {
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
                                    param_type: self.resolve_type(&p.param_type),
                                    span: p.span,
                                })
                                .collect(),
                            self.resolve_type(&func.return_type),
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
            self.metadata.impls.entry(key.clone()).or_insert_with(|| {
                Arc::new(ImplDefinition {
                    key,
                    module: ModuleName::from_str(&sig_module),
                    source_ref: SourceLocation(file_id, Span::dummy()),
                    ast_ref: CheckerAstRef::ModuleParamBinding,
                })
            });
        }
    }

    fn get_sig_functions(&self, name: &str) -> Option<Vec<SigFunction>> {
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

    #[allow(deprecated)]
    fn collect_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
        module_name: &str,
    ) -> Result<(), TypeError> {
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
                    let resolved = self.resolve_type(&f.field_type);
                    self.validate_type_with_params(&resolved, f.span, file_id, &[])?;
                }
                self.metadata.register_type(type_name, Arc::new(entry));
            }
        }

        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    let resolved_return = self.resolve_type(&func.return_type);
                    self.validate_type_with_params(
                        &resolved_return,
                        func.span,
                        file_id,
                        &func.type_params,
                    )?;
                    for param in &func.parameters {
                        let resolved_param_type = self.resolve_type(&param.param_type);
                        self.validate_type_with_params(
                            &resolved_param_type,
                            param.span,
                            file_id,
                            &func.type_params,
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
                                &self.resolve_type(&p.param_type),
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
                    )?;
                    for param in &ext_func.parameters {
                        self.validate_type_with_params(
                            &param.param_type,
                            param.span,
                            file_id,
                            &ext_func.type_params,
                        )?;
                    }
                    let resolved_params: Vec<_> = ext_func
                        .parameters
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: self.resolve_type(&p.param_type),
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
                    let resolved_return = self.resolve_type(&ext_func.return_type);
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
                            ast_ref: CheckerAstRef::ModuleParamBinding,
                        };
                        self.metadata.impls.insert(key, Arc::new(entry));
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
                    let trait_fns = self.get_trait_functions(trait_name);
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
                        let resolved_return =
                            Self::substitute_self(&self.resolve_type(&func.return_type), type_name);
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

    fn resolve_impl_call(
        &self,
        fn_name: &str,
        arguments: &[Expression],
        env: &TypeEnvironment,
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
                            type_name: TypeName {
                                name: type_name.to_string(),
                                module: mn.clone(),
                            },
                            trait_name: TraitName {
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

    fn build_alias_map(module: &Module) -> HashMap<String, String> {
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

    fn build_alias_to_qualified(&self, module: &Module) -> AliasToQualified {
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

    fn resolve_type(&self, t: &AstType) -> AstType {
        match t {
            AstType::Generic(name) if self.get_struct_fields(name).is_some() => {
                AstType::Struct(name.clone())
            }
            AstType::List(inner) => AstType::List(Box::new(self.resolve_type(inner))),
            AstType::Option(inner) => AstType::Option(Box::new(self.resolve_type(inner))),
            other => other.clone(),
        }
    }

    fn validate_type_with_params(
        &self,
        ast_type: &AstType,
        span: Span,
        file_id: FileId,
        type_params: &[TypeParam],
    ) -> Result<(), TypeError> {
        match ast_type {
            AstType::Unit | AstType::Boolean | AstType::String | AstType::Int => Ok(()),
            AstType::Generic(name) => {
                if name == "Self" || type_params.iter().any(|tp| tp.name == *name) {
                    Ok(())
                } else {
                    Err(TypeError::UnboundTypeParameter {
                        name: name.clone(),
                        span,
                        file_id,
                    })
                }
            }
            AstType::List(inner) | AstType::Option(inner) => {
                self.validate_type_with_params(inner, span, file_id, type_params)
            }
            AstType::Struct(name) => {
                if self.get_struct_fields(name).is_some() {
                    Ok(())
                } else {
                    Err(TypeError::UnsupportedType {
                        type_name: name.clone(),
                        span,
                        file_id,
                    })
                }
            }
        }
    }

    fn unify_type(
        formal: &AstType,
        actual: &AstType,
        subst: &mut HashMap<String, AstType>,
    ) -> bool {
        match formal {
            AstType::Generic(name) => {
                if let Some(bound) = subst.get(name) {
                    bound == actual
                } else {
                    subst.insert(name.clone(), actual.clone());
                    true
                }
            }
            AstType::List(inner_formal) => {
                if let AstType::List(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            AstType::Option(inner_formal) => {
                if let AstType::Option(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            _ => formal == actual,
        }
    }

    fn apply_subst(ty: &AstType, subst: &HashMap<String, AstType>) -> AstType {
        match ty {
            AstType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            AstType::List(inner) => AstType::List(Box::new(Self::apply_subst(inner, subst))),
            AstType::Option(inner) => AstType::Option(Box::new(Self::apply_subst(inner, subst))),
            other => other.clone(),
        }
    }

    fn check_function(
        &self,
        func: &Function,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Function, TypeError> {
        let mut env = TypeEnvironment::new();
        for param in &func.parameters {
            env.declare_variable(
                param.name.clone(),
                self.resolve_type(&param.param_type),
                param.span,
            );
        }
        let resolved_return_type = self.resolve_type(&func.return_type);
        let mut typed_stmts = Vec::new();
        for statement in &func.body.statements {
            let (typed_stmt, new_env) =
                self.check_statement(statement, env, &func.name, &resolved_return_type, ctx)?;
            typed_stmts.push(typed_stmt);
            env = new_env;
        }
        Ok(typed_ast::Function {
            name: func.name.clone(),
            parameters: func.parameters.clone(),
            return_type: func.return_type.clone(),
            body: typed_ast::FunctionBody {
                statements: typed_stmts,
                span: func.body.span,
            },
            documentation: func.documentation.clone(),
            is_pub: func.is_pub,
            span: func.span,
        })
    }

    fn check_statement(
        &self,
        statement: &Statement,
        mut env: TypeEnvironment,
        function_name: &str,
        return_type: &AstType,
        ctx: &CheckContext,
    ) -> Result<(typed_ast::Statement, TypeEnvironment), TypeError> {
        match statement {
            Statement::Injection(expr) => {
                let typed_expr = self.check_expression(expr, &env, ctx)?;
                Ok((typed_ast::Statement::Injection(typed_expr), env))
            }
            Statement::Assignment {
                variable,
                expression,
                span,
            } => {
                let typed_expr = self.check_expression(expression, &env, ctx)?;
                let expr_type = typed_expr.ty().clone();
                env.declare_variable(variable.clone(), expr_type, expression.span());
                Ok((
                    typed_ast::Statement::Assignment {
                        variable: variable.clone(),
                        expression: typed_expr,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::VariableAssignment {
                variable,
                expression,
                span,
            } => {
                let typed_expr = self.check_expression(expression, &env, ctx)?;
                let expr_type = typed_expr.ty().clone();
                let (existing_type, declaration_span) = env
                    .lookup_variable_with_span(variable)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: variable.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    })?;

                if expr_type != existing_type {
                    return Err(TypeError::VariableTypeMismatch {
                        variable: variable.clone(),
                        expected: format!("{}", existing_type),
                        found: format!("{}", expr_type),
                        span: expression.span(),
                        declaration_span,
                        file_id: ctx.file_id,
                    });
                }
                Ok((
                    typed_ast::Statement::VariableAssignment {
                        variable: variable.clone(),
                        expression: typed_expr,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::ExpressionStatement(expr) => {
                let typed_expr = self.check_expression(expr, &env, ctx)?;
                Ok((typed_ast::Statement::ExpressionStatement(typed_expr), env))
            }
            Statement::If {
                condition,
                body,
                else_body,
                span,
            } => {
                let typed_condition = self.check_boolean_condition(condition, &env, ctx)?;
                let typed_body =
                    self.check_block(body, env.create_child(), function_name, return_type, ctx)?;
                let typed_else = if let Some(else_stmts) = else_body {
                    Some(self.check_block(
                        else_stmts,
                        env.create_child(),
                        function_name,
                        return_type,
                        ctx,
                    )?)
                } else {
                    None
                };
                Ok((
                    typed_ast::Statement::If {
                        condition: typed_condition,
                        body: typed_body,
                        else_body: typed_else,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::While {
                condition,
                body,
                span,
            } => {
                let typed_condition = self.check_boolean_condition(condition, &env, ctx)?;
                let typed_body =
                    self.check_block(body, env.create_child(), function_name, return_type, ctx)?;
                Ok((
                    typed_ast::Statement::While {
                        condition: typed_condition,
                        body: typed_body,
                        span: *span,
                    },
                    env,
                ))
            }
            Statement::Return(expr) => {
                let typed_expr = self.check_expression(expr, &env, ctx)?;

                if *typed_expr.ty() != *return_type {
                    return Err(TypeError::ReturnTypeMismatch {
                        function: function_name.to_string(),
                        expected: format!("{}", return_type),
                        found: format!("{}", typed_expr.ty()),
                        span: expr.span(),
                        file_id: ctx.file_id,
                    });
                }
                Ok((typed_ast::Statement::Return(typed_expr), env))
            }
        }
    }

    fn check_boolean_condition(
        &self,
        condition: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let typed_cond = self.check_expression(condition, env, ctx)?;
        if matches!(typed_cond.ty(), AstType::Boolean) {
            Ok(typed_cond)
        } else {
            Err(TypeError::TypeMismatch {
                expected: "Boolean".to_string(),
                found: format!("{}", typed_cond.ty()),
                span: condition.span(),
                file_id: ctx.file_id,
            })
        }
    }

    fn check_block(
        &self,
        stmts: &[Statement],
        mut env: TypeEnvironment,
        function_name: &str,
        return_type: &AstType,
        ctx: &CheckContext,
    ) -> Result<Vec<typed_ast::Statement>, TypeError> {
        let mut typed_stmts = Vec::new();
        for stmt in stmts {
            let (typed_stmt, new_env) =
                self.check_statement(stmt, env, function_name, return_type, ctx)?;
            typed_stmts.push(typed_stmt);
            env = new_env;
        }
        Ok(typed_stmts)
    }

    fn check_expression(
        &self,
        expression: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        match expression {
            Expression::Call {
                function,
                arguments,
                span,
            } => self.check_call(function, arguments, *span, env, ctx),
            Expression::Variable { name, span } => {
                let ty = env
                    .lookup_variable(name)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: name.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    })?;
                Ok(typed_ast::Expression::Variable {
                    name: name.clone(),
                    ty,
                    span: *span,
                })
            }
            Expression::StringLiteral { value, span } => Ok(typed_ast::Expression::StringLiteral {
                value: value.clone(),
                ty: AstType::String,
                span: *span,
            }),
            Expression::BooleanLiteral { value, span } => {
                Ok(typed_ast::Expression::BooleanLiteral {
                    value: *value,
                    ty: AstType::Boolean,
                    span: *span,
                })
            }
            Expression::IntLiteral { value, span } => Ok(typed_ast::Expression::IntLiteral {
                value: *value,
                ty: AstType::Int,
                span: *span,
            }),
            Expression::UnitLiteral { span } => Ok(typed_ast::Expression::UnitLiteral {
                ty: AstType::Unit,
                span: *span,
            }),
            Expression::Placeholder { span } => Err(TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                span: *span,
                file_id: ctx.file_id,
            }),
            Expression::ListLiteral { elements, span } => {
                self.check_list_literal(elements, *span, env, ctx)
            }
            Expression::Select(select_expr) => {
                self.check_select(&select_expr.clauses, select_expr.span, env, ctx)
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                span,
            } => self.check_if_else_expression(condition, then_expr, else_expr, *span, env, ctx),
            Expression::StructLiteral {
                struct_name,
                fields,
                span,
            } => self.check_struct_literal(struct_name, fields, *span, env, ctx),
            Expression::FieldAccess { base, field, span } => {
                self.check_field_access(base, field, *span, env, ctx)
            }
        }
    }

    fn check_call(
        &self,
        function: &str,
        arguments: &[Expression],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let resolved = ctx
            .alias_map
            .get(function)
            .map(String::as_str)
            .unwrap_or(function);

        let qualified_for_vis = ctx
            .alias_to_qualified
            .get(function)
            .or_else(|| ctx.alias_to_qualified.get(resolved))
            .map(String::as_str)
            .unwrap_or(resolved);

        self.check_visibility(qualified_for_vis, resolved, span, ctx)?;

        let (mut resolved_fn_name, mut kind, return_type, parameters, type_params) =
            if let Some(sig) = self.lookup_sig(resolved, ctx) {
                let fn_name = Self::make_function_name(resolved, ctx, &sig.kind);
                (
                    fn_name,
                    sig.kind,
                    sig.return_type,
                    sig.parameters,
                    sig.type_params,
                )
            } else if let Some((fn_name, sig)) =
                self.resolve_impl_call(function, arguments, env, ctx)
            {
                (
                    fn_name,
                    sig.kind,
                    sig.return_type,
                    sig.parameters,
                    sig.type_params,
                )
            } else {
                return Err(TypeError::UnknownFunction {
                    name: function.to_string(),
                    span,
                    file_id: ctx.file_id,
                });
            };

        let module_str = resolved_fn_name.module.to_string();
        if let Some(param) = ctx.module_params.iter().find(|p| p.name == module_str)
            && param.path.len() >= 2
        {
            let sig_module = param.path[0].clone();
            let sig_name = param.path.last().unwrap().clone();
            let type_name = TypeName {
                name: param.name.clone(),
                module: ModuleName::from_str("__param__"),
            };
            let trait_name = TraitName {
                name: sig_name,
                module: ModuleName::from_str(&sig_module),
            };
            if let Some(impl_def) = self.metadata.impl_for(&type_name, &trait_name) {
                let lookup_key = FunctionName {
                    name: resolved_fn_name.name.clone(),
                    module: impl_def.module.clone(),
                    kind: FunctionNameKind::Function,
                };
                resolved_fn_name.module = impl_def.module.clone();
                if let Some(fn_def) = self.metadata.function(&lookup_key) {
                    match &fn_def.ast_ref {
                        CheckerAstRef::Function(_, k) => kind = k.clone(),
                        CheckerAstRef::ExternalFn { kind: k, .. } => kind = k.clone(),
                        _ => {}
                    }
                }
            }
        }

        if arguments.len() != parameters.len() {
            return Err(TypeError::ArgumentCountMismatch {
                function: function.to_string(),
                expected: parameters.len(),
                found: arguments.len(),
                span,
                file_id: ctx.file_id,
            });
        }

        let mut typed_args = Vec::new();

        if type_params.is_empty() {
            for (arg, param) in arguments.iter().zip(&parameters) {
                if matches!(arg, Expression::Placeholder { .. }) {
                    typed_args.push(typed_ast::Expression::Placeholder {
                        ty: param.param_type.clone(),
                        span: arg.span(),
                    });
                    continue;
                }
                let typed_arg = self.check_expression(arg, env, ctx)?;
                if typed_arg.ty() != &param.param_type {
                    return Err(TypeError::ArgumentTypeMismatch {
                        function: function.to_string(),
                        parameter: param.name.clone(),
                        expected: format!("{}", param.param_type),
                        found: format!("{}", typed_arg.ty()),
                        span: arg.span(),
                        file_id: ctx.file_id,
                    });
                }
                typed_args.push(typed_arg);
            }

            Ok(typed_ast::Expression::Call {
                function: function.to_string(),
                resolved: resolved_fn_name,
                kind,
                arguments: typed_args,
                ty: return_type,
                span,
            })
        } else {
            let mut subst: HashMap<String, AstType> = HashMap::new();
            for (arg, param) in arguments.iter().zip(&parameters) {
                if matches!(arg, Expression::Placeholder { .. }) {
                    typed_args.push(typed_ast::Expression::Placeholder {
                        ty: param.param_type.clone(),
                        span: arg.span(),
                    });
                    continue;
                }
                let typed_arg = self.check_expression(arg, env, ctx)?;
                if !Self::unify_type(&param.param_type, typed_arg.ty(), &mut subst) {
                    let expected = Self::apply_subst(&param.param_type, &subst);
                    return Err(TypeError::ArgumentTypeMismatch {
                        function: function.to_string(),
                        parameter: param.name.clone(),
                        expected: format!("{}", expected),
                        found: format!("{}", typed_arg.ty()),
                        span: arg.span(),
                        file_id: ctx.file_id,
                    });
                }
                typed_args.push(typed_arg);
            }

            for tp in &type_params {
                if tp.bounds.is_empty() {
                    continue;
                }
                if let Some(concrete) = subst.get(&tp.name) {
                    let type_name = match concrete {
                        AstType::Int => "Int",
                        AstType::String => "String",
                        AstType::Boolean => "Boolean",
                        AstType::Struct(n) => n.as_str(),
                        _ => continue,
                    };
                    for bound in &tp.bounds {
                        let satisfied = self.type_implements_trait(type_name, bound);
                        if !satisfied {
                            return Err(TypeError::TraitBoundNotSatisfied {
                                type_name: type_name.to_string(),
                                trait_name: bound.clone(),
                                param_name: tp.name.clone(),
                                span,
                                file_id: ctx.file_id,
                            });
                        }
                    }
                }
            }

            let resolved_return = Self::apply_subst(&return_type, &subst);
            Ok(typed_ast::Expression::Call {
                function: function.to_string(),
                resolved: resolved_fn_name,
                kind,
                arguments: typed_args,
                ty: resolved_return,
                span,
            })
        }
    }

    fn check_visibility(
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

    fn check_list_literal(
        &self,
        elements: &[Expression],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        if elements.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "non-empty list or type annotation".to_string(),
                found: "empty list".to_string(),
                span,
                file_id: ctx.file_id,
            });
        }

        let typed_first = self.check_expression(&elements[0], env, ctx)?;
        let first_type = typed_first.ty().clone();
        let mut typed_elements = vec![typed_first];

        for elem in elements.iter().skip(1) {
            let typed_elem = self.check_expression(elem, env, ctx)?;
            if *typed_elem.ty() != first_type {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{}", first_type),
                    found: format!("{}", typed_elem.ty()),
                    span: elem.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_elements.push(typed_elem);
        }

        Ok(typed_ast::Expression::ListLiteral {
            elements: typed_elements,
            ty: AstType::List(Box::new(first_type)),
            span,
        })
    }

    fn check_select(
        &self,
        clauses: &[SelectClause],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        if clauses.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "non-empty select".to_string(),
                found: "empty select".to_string(),
                span,
                file_id: ctx.file_id,
            });
        }

        let first = &clauses[0];
        let typed_first_run = self.check_expression(&first.expression_to_run, env, ctx)?;
        let first_result_type = typed_first_run.ty().clone();
        let mut first_env = env.create_child();
        first_env.declare_variable(
            first.result_variable.clone(),
            first_result_type,
            first.expression_to_run.span(),
        );
        let typed_first_next = self.check_expression(&first.expression_next, &first_env, ctx)?;
        let first_type = typed_first_next.ty().clone();

        let mut typed_clauses = vec![typed_ast::SelectClause {
            expression_to_run: typed_first_run,
            result_variable: first.result_variable.clone(),
            expression_next: typed_first_next,
            span: first.span,
        }];

        for (i, clause) in clauses.iter().enumerate().skip(1) {
            let typed_run = self.check_expression(&clause.expression_to_run, env, ctx)?;
            let result_type = typed_run.ty().clone();
            let mut clause_env = env.create_child();
            clause_env.declare_variable(
                clause.result_variable.clone(),
                result_type,
                clause.expression_to_run.span(),
            );
            let typed_next = self.check_expression(&clause.expression_next, &clause_env, ctx)?;
            if first_type != *typed_next.ty() {
                return Err(TypeError::SelectBranchTypeMismatch {
                    expected: format!("{}", first_type),
                    found: format!("{}", typed_next.ty()),
                    branch_index: i,
                    span: clause.expression_next.span(),
                    first_branch_span: first.expression_next.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_clauses.push(typed_ast::SelectClause {
                expression_to_run: typed_run,
                result_variable: clause.result_variable.clone(),
                expression_next: typed_next,
                span: clause.span,
            });
        }

        Ok(typed_ast::Expression::Select(
            typed_ast::SelectExpression {
                clauses: typed_clauses,
                span,
            },
            first_type,
        ))
    }

    fn check_if_else_expression(
        &self,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let typed_condition = self.check_boolean_condition(condition, env, ctx)?;
        let typed_then = self.check_expression(then_expr, env, ctx)?;
        let typed_else = self.check_expression(else_expr, env, ctx)?;

        if typed_then.ty() != typed_else.ty() {
            return Err(TypeError::TypeMismatch {
                expected: format!("{}", typed_then.ty()),
                found: format!("{}", typed_else.ty()),
                span: else_expr.span(),
                file_id: ctx.file_id,
            });
        }

        let ty = typed_then.ty().clone();
        Ok(typed_ast::Expression::IfElse {
            condition: Box::new(typed_condition),
            then_expr: Box::new(typed_then),
            else_expr: Box::new(typed_else),
            ty,
            span,
        })
    }

    fn check_struct_literal(
        &self,
        struct_name: &str,
        fields: &[(String, Expression)],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let definition =
            self.get_struct_fields(struct_name)
                .ok_or_else(|| TypeError::UnsupportedType {
                    type_name: struct_name.to_string(),
                    span,
                    file_id: ctx.file_id,
                })?;

        let mut seen = std::collections::HashSet::new();
        let mut typed_fields = Vec::new();

        for (field_name, value_expr) in fields {
            if !seen.insert(field_name.clone()) {
                return Err(TypeError::DuplicateField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                });
            }

            let declared_type = definition
                .iter()
                .find(|(n, _)| n == field_name)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| TypeError::UnknownField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                })?;

            let typed_value = self.check_expression(value_expr, env, ctx)?;
            if typed_value.ty() != &declared_type {
                return Err(TypeError::StructFieldTypeMismatch {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    expected: format!("{}", declared_type),
                    found: format!("{}", typed_value.ty()),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                });
            }
            typed_fields.push((field_name.clone(), typed_value));
        }

        for (required_field, _) in &definition {
            if !fields.iter().any(|(n, _)| n == required_field) {
                return Err(TypeError::MissingField {
                    struct_name: struct_name.to_string(),
                    field_name: required_field.clone(),
                    span,
                    file_id: ctx.file_id,
                });
            }
        }

        Ok(typed_ast::Expression::StructLiteral {
            struct_name: struct_name.to_string(),
            fields: typed_fields,
            ty: AstType::Struct(struct_name.to_string()),
            span,
        })
    }

    fn check_field_access(
        &self,
        base: &Expression,
        field: &str,
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<typed_ast::Expression, TypeError> {
        let typed_base = self.check_expression(base, env, ctx)?;
        let base_type = typed_base.ty().clone();
        match base_type {
            AstType::Struct(name) | AstType::Generic(name) => {
                let definition =
                    self.get_struct_fields(&name)
                        .ok_or_else(|| TypeError::UnsupportedType {
                            type_name: name.clone(),
                            span,
                            file_id: ctx.file_id,
                        })?;
                let field_type = definition
                    .iter()
                    .find(|(n, _)| n == field)
                    .map(|(_, t)| t.clone())
                    .ok_or_else(|| TypeError::UnknownField {
                        struct_name: name.clone(),
                        field_name: field.to_string(),
                        span,
                        file_id: ctx.file_id,
                    })?;
                Ok(typed_ast::Expression::FieldAccess {
                    base: Box::new(typed_base),
                    field: field.to_string(),
                    ty: field_type,
                    span,
                })
            }
            other => Err(TypeError::TypeMismatch {
                expected: "struct".to_string(),
                found: format!("{}", other),
                span,
                file_id: ctx.file_id,
            }),
        }
    }

    #[allow(deprecated)]
    fn lookup_sig(&self, resolved: &str, ctx: &CheckContext) -> Option<FunctionSignature> {
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
    fn make_function_name(resolved: &str, ctx: &CheckContext, kind: &FunctionKind) -> FunctionName {
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

    fn substitute_self(ty: &AstType, concrete: &str) -> AstType {
        match ty {
            AstType::Generic(name) if name == "Self" => AstType::Struct(concrete.to_string()),
            AstType::List(inner) => AstType::List(Box::new(Self::substitute_self(inner, concrete))),
            AstType::Option(inner) => {
                AstType::Option(Box::new(Self::substitute_self(inner, concrete)))
            }
            other => other.clone(),
        }
    }

    fn substitute_self_in_fn(func: &Function, concrete: &str) -> Function {
        Function {
            name: func.name.clone(),
            parameters: func
                .parameters
                .iter()
                .map(|p| Parameter {
                    name: p.name.clone(),
                    param_type: Self::substitute_self(&p.param_type, concrete),
                    span: p.span,
                })
                .collect(),
            return_type: Self::substitute_self(&func.return_type, concrete),
            body: func.body.clone(),
            documentation: func.documentation.clone(),
            is_pub: func.is_pub,
            span: func.span,
            type_params: func.type_params.clone(),
        }
    }
}

impl TypeChecker {
    pub fn function_kinds(&self) -> HashMap<String, FunctionKind> {
        Self::function_kinds_from_metadata(&self.metadata)
    }

    fn function_kinds_from_metadata(
        metadata: &MetaData<CheckerRefs>,
    ) -> HashMap<String, FunctionKind> {
        metadata
            .all_functions()
            .into_iter()
            .filter_map(|f| match &f.ast_ref {
                CheckerAstRef::Function(_, kind) | CheckerAstRef::ExternalFn { kind, .. } => {
                    Some((f.name.to_string(), kind.clone()))
                }
                _ => None,
            })
            .collect()
    }
}

impl TypeEnvironment {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
        }
    }

    fn create_child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    fn declare_variable(&mut self, name: String, var_type: AstType, span: Span) {
        self.variables.insert(name, (var_type, span));
    }

    fn lookup_variable(&self, name: &str) -> Option<AstType> {
        if let Some((ty, _)) = self.variables.get(name) {
            Some(ty.clone())
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable(name)
        } else {
            None
        }
    }

    fn lookup_variable_with_span(&self, name: &str) -> Option<(AstType, Span)> {
        if let Some((ty, span)) = self.variables.get(name) {
            Some((ty.clone(), *span))
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable_with_span(name)
        } else {
            None
        }
    }
}
