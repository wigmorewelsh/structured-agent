use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};

use crate::constraint::{Constraint, ConstraintKind};
use crate::lattice::TrustLattice;
use crate::types::{Label, LabelExpr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveResult {
    Solved,
    Deferred,
    Conflict(String),
    Emits(Vec<Constraint>),
}

pub struct SolverState {
    pub lattice: TrustLattice,
    pub inert: Vec<Constraint>,
    pub substitution: HashMap<String, Label>,
}

impl SolverState {
    pub fn apply_subst(&self, expr: LabelExpr) -> LabelExpr {
        match expr {
            LabelExpr::Var(ref v) => match self.substitution.get(v) {
                Some(label) => LabelExpr::Concrete(label.clone()),
                None => expr,
            },
            other => other,
        }
    }

    pub fn canonicalise(&self, c: Constraint) -> Constraint {
        match c.kind {
            ConstraintKind::LabelFlow { from, to } => Constraint {
                kind: ConstraintKind::LabelFlow {
                    from: self.apply_subst(from),
                    to: self.apply_subst(to),
                },
                ..c
            },
            _ => c,
        }
    }

    pub fn derive_from_inert(&self, var: &str, label: &Label) -> Vec<Constraint> {
        let mut derived = Vec::new();
        for c in &self.inert {
            if let ConstraintKind::LabelFlow { from, to } = &c.kind {
                let new_from = match from {
                    LabelExpr::Var(v) if v == var => LabelExpr::Concrete(label.clone()),
                    other => other.clone(),
                };
                let new_to = match to {
                    LabelExpr::Var(v) if v == var => LabelExpr::Concrete(label.clone()),
                    other => other.clone(),
                };
                if new_from != *from || new_to != *to {
                    derived.push(Constraint::derived(ConstraintKind::LabelFlow {
                        from: new_from,
                        to: new_to,
                    }));
                }
            }
        }
        derived
    }
}

pub trait SolveRule {
    fn applies(&self, constraint: &Constraint) -> bool;
    fn apply(&self, constraint: &Constraint, state: &mut SolverState) -> SolveResult;
}

pub struct Solver {
    state: SolverState,
    pub errors: Vec<String>,
    worklist: VecDeque<Constraint>,
    rules: Vec<Box<dyn SolveRule>>,
}

impl Deref for Solver {
    type Target = SolverState;

    fn deref(&self) -> &SolverState {
        &self.state
    }
}

impl DerefMut for Solver {
    fn deref_mut(&mut self) -> &mut SolverState {
        &mut self.state
    }
}

impl Solver {
    pub fn new() -> Self {
        use crate::rules::{LabelFlowRule, LabelUnifyRule};
        Self {
            state: SolverState {
                lattice: TrustLattice::new(),
                inert: Vec::new(),
                substitution: HashMap::new(),
            },
            errors: Vec::new(),
            worklist: VecDeque::new(),
            rules: vec![Box::new(LabelFlowRule), Box::new(LabelUnifyRule)],
        }
    }

    pub fn add_wanted(&mut self, kind: ConstraintKind) {
        self.worklist.push_back(Constraint::wanted(kind));
    }

    pub fn add_given(&mut self, kind: ConstraintKind) {
        self.state.inert.push(Constraint::given(kind));
    }

    pub fn step(&mut self) -> Option<SolveResult> {
        let c = self.worklist.pop_front()?;
        let c = self.state.canonicalise(c);

        let idx = self.rules.iter().position(|r| r.applies(&c));
        let result = match idx {
            Some(i) => {
                let rules = &self.rules;
                let state = &mut self.state;
                rules[i].apply(&c, state)
            }
            None => SolveResult::Deferred,
        };

        match &result {
            SolveResult::Conflict(msg) => self.errors.push(msg.clone()),
            SolveResult::Deferred => self.state.inert.push(c),
            SolveResult::Emits(new_cs) => {
                for nc in new_cs {
                    self.worklist.push_back(nc.clone());
                }
            }
            SolveResult::Solved => {}
        }

        Some(result)
    }

    pub fn solve(&mut self) {
        while self.step().is_some() {}
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}
