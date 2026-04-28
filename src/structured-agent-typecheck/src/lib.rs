mod collection;
mod db;
mod elaboration;
mod error;
mod refs;
mod solver;
mod synthesize;

#[cfg(test)]
mod tests;

pub use error::TypeError;
pub use error::TypeErrorAccumulator;
pub use refs::{
    CheckerAstRef, CheckerRefs, FunctionKind, ModuleVisibility, NoBody, NoWitness, PrimitiveRefs,
    SourceLocation, TypedCheckerAstRef, TypedRefs,
};

pub(crate) use structured_agent_ast::ast;
pub(crate) use structured_agent_ast::types;
pub(crate) use structured_agent_typed_ast as typed_ast;

use crate::db::ProgramInput;
use collection::SymbolTableBuilder;
use db::{ParsedModuleInput, TypeCheckDatabase, TypeCheckDb};
use structured_agent_ast::ast::{ParsedModule, TypeParam};

use std::collections::HashMap;
use std::sync::Arc;
use structured_agent_il::Module as RuntimeModule;
use structured_agent_runtime::symbols::MetaData;

pub struct TypeChecker {
    db: TypeCheckDb,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSignature {
    pub parameters: Vec<typed_ast::Parameter>,
    pub return_type: structured_agent_runtime::Type,
    pub kind: FunctionKind,
    pub type_params: Vec<TypeParam>,
}

pub use synthesize::{CheckContext, TypeEnvironment};

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            db: TypeCheckDb::default(),
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

        let check_errors = self.run_check_pass(program_input);
        if !check_errors.is_empty() {
            return Err(check_errors);
        }

        let solver_errors = self.run_solve_pass(program_input);
        if !solver_errors.is_empty() {
            return Err(solver_errors);
        }

        let (meta_data, elaborate_errors) = self.run_elaborate_pass(program_input);
        if !elaborate_errors.is_empty() {
            return Err(elaborate_errors);
        }

        Ok(meta_data)
    }

    fn run_elaborate_pass(
        &mut self,
        program_input: ProgramInput,
    ) -> (MetaData<TypedRefs>, Vec<TypeError>) {
        let meta_data = db::elaborate_metadata(&self.db, program_input)
            .get()
            .clone();

        let errors =
            db::elaborate_metadata::accumulated::<TypeErrorAccumulator>(&self.db, program_input)
                .into_iter()
                .map(|e| e.0.clone())
                .collect::<Vec<_>>();

        (meta_data, errors)
    }

    fn run_solve_pass(&mut self, program_input: ProgramInput) -> Vec<TypeError> {
        let _ = solver::solve_constraints(&self.db, program_input);

        solver::solve_constraints::accumulated::<TypeErrorAccumulator>(&self.db, program_input)
            .into_iter()
            .map(|e| e.0.clone())
            .collect()
    }

    fn populate_symbol_tables(
        &mut self,
        modules: &[ParsedModule],
        native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
    ) {
        let tables =
            SymbolTableBuilder::new().build_symbol_tables(&self.db, modules, native_modules);
        self.db.set_symbol_tables(tables);
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
        db::check_program(&self.db, program);

        db::check_program::accumulated::<TypeErrorAccumulator>(&self.db, program)
            .into_iter()
            .map(|e| e.0.clone())
            .collect()
    }

    pub fn function_kinds(&self) -> HashMap<String, FunctionKind> {
        self.db
            .symbol_tables()
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
