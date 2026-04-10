mod collection;
mod constraints;
mod db;
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

use crate::ast::{Parameter, ParsedModule, Type as AstType, TypeParam};
use crate::typed_ast;
use crate::types::{FileId, Span};
use collection::SymbolTableBuilder;
use db::{ParsedModuleInput, SymbolTablesInput, TypeCheckDb};
use nonempty::NonEmpty;

use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    FieldDefinition, FunctionDefinition, GenericParameterDefinition, ImplDefinition, MetaData,
    ModuleDefinition, ModuleName, ParameterDefinition, SignatureEntry, SymbolQuery, TypeDefinition,
    TypeDefinitionKind, TypeName,
};
use structured_agent_runtime::types::Module as RuntimeModule;

pub struct TypeChecker {
    metadata: MetaData<CheckerRefs>,
    db: TypeCheckDb,
    symbol_tables: Option<SymbolTablesInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub(super) module_name: &'a ModuleName,
}

pub(super) fn ast_type_to_type_name(ty: &AstType, module_name: &ModuleName) -> TypeName {
    match ty {
        AstType::Struct(name) => TypeName {
            name: name.clone(),
            module: module_name.clone(),
        },
        AstType::List(_) => TypeName {
            name: "List".to_string(),
            module: ModuleName::new(NonEmpty::new("prelude".to_string())),
        },
        AstType::Option(_) => TypeName {
            name: "Option".to_string(),
            module: ModuleName::new(NonEmpty::new("prelude".to_string())),
        },
        other => TypeName {
            name: other.to_string(),
            module: ModuleName::new(NonEmpty::new("prelude".to_string())),
        },
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_type_kind(
    kind: &TypeDefinitionKind<refs::CheckerRefs>,
    module: &ModuleName,
) -> TypeDefinitionKind<refs::TypedRefs> {
    match kind {
        TypeDefinitionKind::Struct { fields } => TypeDefinitionKind::Struct {
            fields: fields
                .iter()
                .map(|f| FieldDefinition {
                    name: f.name.clone(),
                    type_name: ast_type_to_type_name(&f.type_name, module),
                })
                .collect(),
        },
        TypeDefinitionKind::Function {
            parameters,
            generic_parameters,
            return_type,
        } => TypeDefinitionKind::Function {
            parameters: parameters
                .iter()
                .map(|p| ParameterDefinition {
                    name: p.name.clone(),
                    type_name: ast_type_to_type_name(&p.type_name, module),
                })
                .collect(),
            generic_parameters: generic_parameters
                .iter()
                .map(|gp| GenericParameterDefinition {
                    name: gp.name.clone(),
                    constraints: gp
                        .constraints
                        .iter()
                        .map(|c| ast_type_to_type_name(c, module))
                        .collect(),
                })
                .collect(),
            return_type: ast_type_to_type_name(return_type, module),
        },
        TypeDefinitionKind::Signature { entries } => TypeDefinitionKind::Signature {
            entries: entries
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: ast_type_to_type_name(&e.type_name, module),
                })
                .collect(),
        },
        TypeDefinitionKind::Trait { functions, .. } => TypeDefinitionKind::Trait {
            functions: functions
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: ast_type_to_type_name(&e.type_name, module),
                })
                .collect(),
            witness_ref: NoWitness,
        },
        TypeDefinitionKind::Primitive => TypeDefinitionKind::Primitive,
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            metadata: MetaData::default(),
            db: TypeCheckDb::default(),
            symbol_tables: None,
        }
    }

    pub fn check_modules(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) -> Result<
        (
            MetaData<TypedRefs>,
            HashMap<NonEmpty<String>, typed_ast::Module>,
        ),
        TypeError,
    > {
        self.populate_symbol_tables(modules, native_modules)?;
        let typed_modules = self.typecheck_modules(modules)?;
        let typed_metadata = self.materialize_metadata(modules, &typed_modules);
        Ok((typed_metadata, typed_modules))
    }

    fn populate_symbol_tables(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) -> Result<(), TypeError> {
        let (metadata, tables) =
            SymbolTableBuilder::new().build_symbol_tables(&self.db, modules, native_modules);
        self.metadata = metadata;
        self.symbol_tables = Some(tables);
        Ok(())
    }

    fn typecheck_modules(
        &mut self,
        modules: &[ParsedModule],
    ) -> Result<HashMap<NonEmpty<String>, typed_ast::Module>, TypeError> {
        let tables = self.symbol_tables.expect("symbol tables not populated");
        let mut typed_modules = HashMap::new();
        for parsed in modules {
            let parsed_input = ParsedModuleInput::new(
                &self.db,
                parsed.name.clone(),
                parsed.is_entry,
                parsed.file_id,
                parsed.module.clone(),
            );
            let arc_module = db::check_module(&self.db, parsed_input, tables)?;
            typed_modules.insert(parsed.name.clone(), arc_module.get().clone());
        }
        Ok(typed_modules)
    }

    fn materialize_metadata(
        &self,
        modules: &[ParsedModule],
        typed_modules: &HashMap<NonEmpty<String>, typed_ast::Module>,
    ) -> MetaData<TypedRefs> {
        let effective_name_to_typed: HashMap<ModuleName, &typed_ast::Module> = modules
            .iter()
            .map(|p| {
                let eff = if p.is_entry {
                    ModuleName::new(NonEmpty::new("main".to_string()))
                } else {
                    ModuleName::new(p.name.clone())
                };
                (eff, typed_modules.get(&p.name).unwrap())
            })
            .collect();
        let mut typed_metadata: MetaData<TypedRefs> = MetaData::default();
        for fn_def in self.metadata.all_functions() {
            let typed_ast_ref = match &fn_def.ast_ref {
                CheckerAstRef::Function(_, kind) => {
                    let typed_module = effective_name_to_typed[&fn_def.name.module];
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
                    let typed_module = effective_name_to_typed[&fn_def.name.module];
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
            let kind = convert_type_kind(&type_def.kind, &type_def.name.module);
            let new_def = TypeDefinition {
                name: type_def.name.clone(),
                kind,
                source_ref: SourceLocation(type_def.source_ref.0, type_def.source_ref.1),
                ast_ref: TypedCheckerAstRef::Other(type_def.ast_ref.clone()),
            };
            typed_metadata
                .types
                .insert(type_def.name.clone(), Arc::new(new_def));
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
                use_imports: module_def.use_imports.clone(),
            };
            typed_metadata
                .modules
                .insert(module_key.clone(), Arc::new(new_def));
        }
        typed_metadata
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
