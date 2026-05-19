use nonempty::NonEmpty;
use salsa::Accumulator;
use std::collections::{HashMap, VecDeque};

pub mod rules;
use structured_agent_ast::CheckerAstRef;
use structured_agent_runtime::Type;
use structured_agent_runtime::symbols::DefinitionPath;

use crate::db::{ProgramInput, TypeCheckDatabase, check_module};
use crate::{TypeError, TypeErrorAccumulator};
use structured_agent_ast::types::Span;

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum Flavour {
    Wanted,
    Given,
    Derived,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConstraintKind {
    TypeBound {
        caller: String,
        callee: String,
        actual_type: Type,
        bound_type: Type,
        context: String,
    },

    TraitImpl {
        type_path: DefinitionPath,
        trait_path: DefinitionPath,
        impl_key: DefinitionPath,
    },
    TraitBound {
        type_path: DefinitionPath,
        trait_path: DefinitionPath,
        type_name: String,
        trait_name: String,
        param_name: String,
    },
    Unify {
        call_site: usize,
        var: String,
        ty: Type,
        fn_name: String,
        param_name: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub span: Span,
    pub file_id: usize,
    pub flavour: Flavour,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheckResult {
    pub constraints: Vec<Constraint>,
}

#[derive(Debug)]
pub enum SolveResult {
    Solved,
    Deferred,
    Conflict(TypeError),
    Emits(Vec<Constraint>),
}

pub struct SolverState {
    pub inert: Vec<Constraint>,
    pub generic_solutions: HashMap<(usize, usize), HashMap<String, Type>>,
    pub resolved: HashMap<String, HashMap<String, Vec<Type>>>,
    pub impls: HashMap<(DefinitionPath, DefinitionPath), DefinitionPath>,
}

pub trait SolveRule {
    fn applies(&self, constraint: &Constraint) -> bool;
    fn apply(
        &self,
        constraint: &Constraint,
        db: &dyn TypeCheckDatabase,
        state: &mut SolverState,
    ) -> SolveResult;
}

pub struct Solver {
    pub state: SolverState,
    pub errors: Vec<TypeError>,
    pub worklist: VecDeque<Constraint>,
    pub rules: Vec<Box<dyn SolveRule>>,
}

impl Solver {
    pub fn new() -> Self {
        Self {
            state: SolverState {
                inert: vec![],
                generic_solutions: HashMap::new(),
                resolved: HashMap::new(),
                impls: HashMap::new(),
            },
            errors: Vec::new(),
            worklist: VecDeque::new(),
            rules: vec![
                Box::new(rules::unify::UnifyRule),
                Box::new(rules::type_bound::TypeBoundRule),
                Box::new(rules::trait_impl::TraitImplRule),
                Box::new(rules::trait_bound::TraitBoundRule),
            ],
        }
    }

    pub fn step(&mut self, db: &dyn TypeCheckDatabase) -> Option<SolveResult> {
        let c = self.worklist.pop_front()?;
        let idx = self.rules.iter().position(|r| r.applies(&c));
        let result = match idx {
            Some(i) => self.rules[i].apply(&c, db, &mut self.state),
            None => SolveResult::Deferred,
        };
        match &result {
            SolveResult::Deferred => self.state.inert.push(c),
            SolveResult::Conflict(e) => self.errors.push(e.clone()),
            SolveResult::Emits(cs) => self.worklist.extend(cs.clone()),
            SolveResult::Solved => {}
        }
        Some(result)
    }

    pub fn solve(&mut self, db: &dyn TypeCheckDatabase) {
        while self.step(db).is_some() {}
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SolvedConstraints {
    pub resolved: HashMap<String, HashMap<String, Vec<Type>>>,
    pub impls: HashMap<(DefinitionPath, DefinitionPath), DefinitionPath>,
    pub inherent_impls: HashMap<DefinitionPath, DefinitionPath>,
    pub generic_solutions: HashMap<(usize, usize), HashMap<String, Type>>,
}

#[salsa::tracked]
pub fn solve_constraints(db: &dyn TypeCheckDatabase, program: ProgramInput) -> SolvedConstraints {
    let constraints: Vec<Constraint> = program
        .modules(db)
        .iter()
        .flat_map(|parsed| check_module(db, *parsed, program).constraints)
        .collect();
    let mut solver = Solver::new();

    for constraint in &constraints {
        match &constraint.kind {
            ConstraintKind::TypeBound { .. } => {
                solver.worklist.push_back(constraint.clone());
            }
            ConstraintKind::TraitImpl { .. } => {
                solver.worklist.push_back(constraint.clone());
            }
            ConstraintKind::TraitBound { .. } => {
                solver.worklist.push_back(constraint.clone());
            }
            ConstraintKind::Unify { .. } => {
                solver.worklist.push_back(constraint.clone());
            }
        }
    }

    for (impl_key, impl_def) in db.symbol_tables().impls(db).get().iter() {
        if !matches!(impl_def.ast_ref, CheckerAstRef::Primitive) {
            continue;
        }
        let Some(trait_type) = &impl_def.trait_name else {
            continue;
        };
        let type_name = impl_def.type_name.name().to_string();
        let trait_name = trait_type.name().to_string();
        let local_type = DefinitionPath::for_type(impl_def.module.clone(), &type_name);
        let type_path = if db.symbol_tables().types(db).get().contains_key(&local_type) {
            local_type
        } else {
            DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
                &type_name,
            )
        };
        let trait_path = DefinitionPath::for_type(impl_def.module.clone(), &trait_name);
        solver
            .state
            .impls
            .insert((type_path, trait_path), impl_key.clone());
    }

    solver.solve(db);
    for e in solver.errors {
        TypeErrorAccumulator(e).accumulate(db);
    }

    for c in &solver.state.inert {
        if let ConstraintKind::TraitBound {
            type_name,
            trait_name,
            param_name,
            ..
        } = &c.kind
        {
            TypeErrorAccumulator(TypeError::TraitBoundNotSatisfied {
                type_name: type_name.clone(),
                trait_name: trait_name.clone(),
                param_name: param_name.clone(),
                span: c.span,
                file_id: c.file_id,
            })
            .accumulate(db);
        }
    }

    let mut inherent_impls: HashMap<DefinitionPath, DefinitionPath> = HashMap::new();
    for (impl_key, impl_def) in db.symbol_tables().impls(db).get().iter() {
        if impl_def.trait_name.is_some() {
            continue;
        }
        let type_name = impl_def.type_name.name().to_string();
        let local_type = DefinitionPath::for_type(impl_def.module.clone(), &type_name);
        let type_path = if db.symbol_tables().types(db).get().contains_key(&local_type) {
            local_type
        } else {
            DefinitionPath::for_type(
                DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
                &type_name,
            )
        };
        inherent_impls.insert(type_path, impl_key.clone());
    }

    SolvedConstraints {
        resolved: solver.state.resolved,
        impls: solver.state.impls,
        inherent_impls,
        generic_solutions: solver.state.generic_solutions,
    }
}
