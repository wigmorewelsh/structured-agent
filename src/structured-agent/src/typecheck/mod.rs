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

use crate::ast::{ParsedModule, Type as AstType, TypeParam};
use crate::typed_ast;
use crate::types::{FileId, Span};
use collection::SymbolTableBuilder;
use db::{Intern, ParsedModuleInput, SymbolTablesInput, TypeCheckDatabase, TypeCheckDb};
use nonempty::NonEmpty;

use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    FieldDefinition, FunctionDefinition, GenericParameterDefinition, ImplDefinition, MetaData,
    ModuleDefinition, ModuleName, ParameterDefinition, SignatureEntry, TypeDefinition,
    TypeDefinitionKind, TypeName,
};
use structured_agent_runtime::types::Module as RuntimeModule;

pub struct TypeChecker {
    db: TypeCheckDb,
    symbol_tables: Option<SymbolTablesInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct FunctionSignature {
    pub(super) parameters: Vec<crate::typed_ast::Parameter>,
    pub(super) return_type: structured_agent_runtime::Type,
    pub(super) kind: FunctionKind,
    pub(super) type_params: Vec<TypeParam>,
}

#[derive(Debug, Clone)]
pub(super) struct TypeEnvironment {
    pub(super) variables: HashMap<String, (structured_agent_runtime::Type, Span)>,
    pub(super) parent: Option<Box<TypeEnvironment>>,
}

pub(super) struct CheckContext<'a> {
    pub(super) file_id: FileId,
    pub(super) module_name: &'a ModuleName,
}

pub(super) fn ast_type_to_type_name(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    ty: &AstType,
    module_name: &ModuleName,
) -> TypeName {
    let interned_mod = module_name.intern(db);
    let interned_name = ty.name.clone().intern(db);
    db::resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
        name: ty.name.clone(),
        module: module_name.clone(),
    })
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_generic_params(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    generic_parameters: &[GenericParameterDefinition<refs::CheckerRefs>],
    module: &ModuleName,
) -> Vec<GenericParameterDefinition<refs::TypedRefs>> {
    generic_parameters
        .iter()
        .map(|gp| GenericParameterDefinition {
            name: gp.name.clone(),
            constraints: gp
                .constraints
                .iter()
                .map(|c| ast_type_to_type_name(db, tables, c, module))
                .collect(),
        })
        .collect()
}

fn convert_type_kind(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    kind: &TypeDefinitionKind<refs::CheckerRefs>,
    module: &ModuleName,
) -> TypeDefinitionKind<refs::TypedRefs> {
    match kind {
        TypeDefinitionKind::Struct {
            fields,
            generic_parameters,
        } => TypeDefinitionKind::Struct {
            fields: fields
                .iter()
                .map(|f| FieldDefinition {
                    name: f.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &f.type_name, module),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
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
                    type_name: ast_type_to_type_name(db, tables, &p.type_name, module),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
            return_type: ast_type_to_type_name(db, tables, return_type, module),
        },
        TypeDefinitionKind::Signature { entries } => TypeDefinitionKind::Signature {
            entries: entries
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &e.type_name, module),
                })
                .collect(),
        },
        TypeDefinitionKind::Trait { functions, .. } => TypeDefinitionKind::Trait {
            functions: functions
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &e.type_name, module),
                })
                .collect(),
            witness_ref: NoWitness,
        },
        TypeDefinitionKind::Primitive => TypeDefinitionKind::Primitive,
        TypeDefinitionKind::Native {
            generic_parameters,
            factory,
        } => TypeDefinitionKind::Native {
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
            factory: factory.clone(),
        },
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
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
        let typed_metadata = self.materialize_metadata(&typed_modules);
        Ok((typed_metadata, typed_modules))
    }

    fn populate_symbol_tables(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) -> Result<(), TypeError> {
        self.symbol_tables =
            Some(SymbolTableBuilder::new().build_symbol_tables(&self.db, modules, native_modules));
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
        typed_modules: &HashMap<NonEmpty<String>, typed_ast::Module>,
    ) -> MetaData<TypedRefs> {
        let tables = self.symbol_tables.expect("symbol tables not populated");
        let mut typed_metadata: MetaData<TypedRefs> = MetaData::default();
        for fn_def in tables.functions(&self.db).get().values() {
            let typed_ast_ref = match &fn_def.ast_ref {
                CheckerAstRef::Function(_, kind) => {
                    let typed_module = typed_modules.get(&fn_def.name.module.segments).unwrap();
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
                    let typed_module = typed_modules.get(&fn_def.name.module.segments).unwrap();
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
        for type_def in tables.types(&self.db).get().values() {
            let kind = convert_type_kind(&self.db, tables, &type_def.kind, &type_def.name.module);
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

        for (impl_key, impl_def) in tables.impls(&self.db).get() {
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

        for (module_key, module_def) in tables.modules(&self.db).get() {
            let new_def = ModuleDefinition {
                name: module_def.name.clone(),
                visibility: module_def.visibility.clone(),
                is_entry: module_def.is_entry,
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
        let tables = self.symbol_tables.expect("symbol tables not populated");
        tables
            .functions(&self.db)
            .get()
            .values()
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

    fn declare_variable(
        &mut self,
        name: String,
        var_type: structured_agent_runtime::Type,
        span: Span,
    ) {
        self.variables.insert(name, (var_type, span));
    }

    fn lookup_variable(&self, name: &str) -> Option<structured_agent_runtime::Type> {
        if let Some((ty, _)) = self.variables.get(name) {
            Some(ty.clone())
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable(name)
        } else {
            None
        }
    }

    fn lookup_variable_with_span(
        &self,
        name: &str,
    ) -> Option<(structured_agent_runtime::Type, Span)> {
        if let Some((ty, span)) = self.variables.get(name) {
            Some((ty.clone(), *span))
        } else if let Some(parent) = &self.parent {
            parent.lookup_variable_with_span(name)
        } else {
            None
        }
    }
}
