mod collection;
mod constraints;
mod db;
mod elaboration;
mod error;
mod refs;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_test;

pub use error::TypeError;
pub use error::TypeErrorAccumulator;
pub use refs::{
    AliasToQualified, CheckerAstRef, CheckerRefs, FunctionKind, ModuleVisibility, NoBody,
    NoWitness, PrimitiveRefs, SourceLocation, TypedCheckerAstRef, TypedRefs,
};

use crate::ast::{ParsedModule, TypeParam};
use crate::types::{FileId, Span};
use collection::SymbolTableBuilder;
use db::{ParsedModuleInput, SymbolTablesInput, TypeCheckDb};

use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::{MetaData, ModuleName};
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

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
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
    ) -> Result<MetaData<TypedRefs>, Vec<TypeError>> {
        self.populate_symbol_tables(modules, native_modules);
        let parsed_inputs = self.make_parsed_inputs(modules);
        let errors = self.run_check_pass(&parsed_inputs);
        if !errors.is_empty() {
            return Err(errors);
        }
        let tables = self.symbol_tables.expect("symbol tables not populated");
        Ok(db::elaborate_metadata(&self.db, tables).get().clone())
    }

    fn populate_symbol_tables(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) {
        self.symbol_tables =
            Some(SymbolTableBuilder::new().build_symbol_tables(&self.db, modules, native_modules));
    }

    fn make_parsed_inputs(&mut self, modules: &[ParsedModule]) -> Vec<ParsedModuleInput> {
        modules
            .iter()
            .map(|parsed| {
                ParsedModuleInput::new(
                    &self.db,
                    parsed.name.clone(),
                    parsed.is_entry,
                    parsed.file_id,
                    parsed.module.clone(),
                )
            })
            .collect()
    }

    fn run_check_pass(&self, parsed_inputs: &[ParsedModuleInput]) -> Vec<TypeError> {
        let tables = self.symbol_tables.expect("symbol tables not populated");
        let mut all_errors: Vec<TypeError> = Vec::new();
        for &parsed_input in parsed_inputs {
            db::check_module(&self.db, parsed_input, tables);
            let errors = db::check_module::accumulated::<TypeErrorAccumulator>(
                &self.db,
                parsed_input,
                tables,
            );
            all_errors.extend(errors.into_iter().map(|e| e.0.clone()));
        }
        all_errors
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
