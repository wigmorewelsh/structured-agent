mod collection;
mod db;
mod elaboration;
mod error;
mod refs;
mod solver;
mod synthesize;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_test;

pub use error::TypeError;
pub use error::TypeErrorAccumulator;
pub use refs::{
    CheckerAstRef, CheckerRefs, FunctionKind, ModuleVisibility, NoBody, NoWitness, PrimitiveRefs,
    SourceLocation, TypedCheckerAstRef, TypedRefs,
};

use crate::ast::{ParsedModule, TypeParam};
use crate::typecheck::db::ProgramInput;
use collection::SymbolTableBuilder;
use db::{ParsedModuleInput, SymbolTablesInput, TypeCheckDb};

use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_runtime::symbols::MetaData;
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

pub(super) use synthesize::{CheckContext, TypeEnvironment};

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

    pub fn check(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) -> Result<MetaData<TypedRefs>, Vec<TypeError>> {
        self.populate_symbol_tables(modules, native_modules);
        let parsed_inputs = self.make_parsed_inputs(modules);
        let program_input = ProgramInput::new(&self.db, parsed_inputs);
        let tables = self.symbol_tables.expect("symbol tables not populated");

        let check_errors = self.run_check_pass(program_input);
        if !check_errors.is_empty() {
            return Err(check_errors);
        }

        let solver_errors = self.run_solve_pass(program_input, tables);
        if !solver_errors.is_empty() {
            return Err(solver_errors);
        }

        let (meta_data, elaborate_errors) = self.run_elaborate_pass(program_input, tables);
        if !elaborate_errors.is_empty() {
            return Err(elaborate_errors);
        }

        Ok(meta_data)
    }

    fn run_elaborate_pass(
        &mut self,
        program_input: ProgramInput,
        tables: SymbolTablesInput,
    ) -> (MetaData<TypedRefs>, Vec<TypeError>) {
        let meta_data = db::elaborate_metadata(&self.db, tables, program_input)
            .get()
            .clone();

        let errors = db::elaborate_metadata::accumulated::<TypeErrorAccumulator>(
            &self.db,
            tables,
            program_input,
        )
        .into_iter()
        .map(|e| e.0.clone())
        .collect::<Vec<_>>();

        (meta_data, errors)
    }

    fn run_solve_pass(
        &mut self,
        program_input: ProgramInput,
        tables: SymbolTablesInput,
    ) -> Vec<TypeError> {
        let _ = solver::solve_constraints(&self.db, program_input, tables);

        let solver_errors = solver::solve_constraints::accumulated::<TypeErrorAccumulator>(
            &self.db,
            program_input,
            tables,
        )
        .into_iter()
        .map(|e| e.0.clone())
        .collect();

        solver_errors
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

    fn run_check_pass(&self, program: ProgramInput) -> Vec<TypeError> {
        let tables = self.symbol_tables.expect("symbol tables not populated");

        db::check_program(&self.db, program, tables);

        let all_errors =
            db::check_program::accumulated::<TypeErrorAccumulator>(&self.db, program, tables)
                .into_iter()
                .map(|e| e.0.clone())
                .collect();

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
