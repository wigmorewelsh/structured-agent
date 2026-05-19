use crate::types::{Label, LabelExpr};

// TODO: The flavour is carried on every constraint but the solver treats Derived and Wanted
// identically when checking the lattice. The distinction matters for error reporting:
// Derived conflicts should be suppressed in favour of the root Wanted that caused them.
// Flavour is also unused on Given constraints in the inert set — Givens are added directly
// to the inert set at startup and never re-processed, so their flavour is never inspected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flavour {
    Given,
    Wanted,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintKind {
    LabelFlow { from: LabelExpr, to: LabelExpr },
    LabelUnify { var: String, label: Label },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub flavour: Flavour,
    pub kind: ConstraintKind,
}

impl Constraint {
    pub fn wanted(kind: ConstraintKind) -> Self {
        Self {
            flavour: Flavour::Wanted,
            kind,
        }
    }

    pub fn given(kind: ConstraintKind) -> Self {
        Self {
            flavour: Flavour::Given,
            kind,
        }
    }

    pub fn derived(kind: ConstraintKind) -> Self {
        Self {
            flavour: Flavour::Derived,
            kind,
        }
    }
}
