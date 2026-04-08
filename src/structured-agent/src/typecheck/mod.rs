mod collection;
mod constraints;
mod elaboration;
mod error;
mod query;
mod refs;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_test;

pub use error::TypeError;
pub use refs::{
    AliasToQualified, CheckerAstRef, CheckerRefs, FunctionKind, ModuleVisibility, NoBody,
    NoWitness, PrimitiveRefs, SourceLocation, TypedCheckerAstRef, TypedRefs,
};

use crate::ast::{Definition, ModuleParam, Parameter, ParsedModule, Type as AstType, TypeParam};
use crate::typed_ast;
use crate::types::{FileId, Span};
use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    ExportedName, FunctionDefinition, ImplDefinition, ImplKey, MetaData, ModuleDefinition,
    ModuleName, SymbolQuery, TraitDefinition, TypeDefinition, TypeName, Visibility,
};
use structured_agent_runtime::types::Module as RuntimeModule;

pub struct TypeChecker {
    pub(super) metadata: MetaData<CheckerRefs>,
    pub(super) param_bindings: HashMap<ImplKey, Arc<ImplDefinition<PrimitiveRefs>>>,
    pub(super) primitive_types: HashMap<TypeName, Arc<TypeDefinition<PrimitiveRefs>>>,
}

#[derive(Debug, Clone)]
pub(super) struct FunctionSignature {
    pub(super) parameters: Vec<Parameter>,
    pub(super) return_type: AstType,
    pub(super) kind: FunctionKind,
    pub(super) type_params: Vec<TypeParam>,
}

#[derive(Debug, Clone)]
pub(super) struct TypeEnvironment {
    pub(super) variables: HashMap<String, (AstType, Span)>,
    pub(super) parent: Option<Box<TypeEnvironment>>,
}

pub(super) struct CheckContext<'a> {
    pub(super) file_id: FileId,
    pub(super) alias_map: &'a HashMap<String, String>,
    pub(super) alias_to_qualified: &'a AliasToQualified,
    pub(super) module_name: Option<&'a str>,
    pub(super) module_params: &'a [ModuleParam],
}

pub(super) fn ast_type_to_type_name(ty: &AstType, module_name: &str) -> TypeName {
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

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut checker = Self {
            metadata: MetaData::default(),
            param_bindings: HashMap::new(),
            primitive_types: HashMap::new(),
        };
        checker.seed_builtin_types();
        checker
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
        for (type_key, type_def) in &self.primitive_types {
            let new_def = TypeDefinition {
                name: type_def.name.clone(),
                kind: type_def.kind.clone(),
                source_ref: SourceLocation(type_def.source_ref.0, type_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::NoAst,
            };
            typed_metadata
                .types
                .insert(type_key.clone(), Arc::new(new_def));
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
        for (impl_key, impl_def) in &self.param_bindings {
            let new_def = ImplDefinition {
                key: impl_def.key.clone(),
                module: impl_def.module.clone(),
                source_ref: SourceLocation(impl_def.source_ref.0, impl_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::NoAst,
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
