use std::collections::HashSet;

use crate::types::Label;

// The precomputed HashSet is over-engineered for three levels — satisfies could just be
// value >= requirement directly on the Label enum. The HashSet approach scales to arbitrary
// lattices where the order isn't derivable from enum declaration order, but for a fixed
// trust lattice it adds indirection without benefit.
pub struct TrustLattice {
    pairs: HashSet<(Label, Label)>,
}

impl TrustLattice {
    pub fn new() -> Self {
        let levels = [Label::Untrusted, Label::External, Label::Internal];
        let mut pairs = HashSet::new();
        for value in &levels {
            for req in &levels {
                if value >= req {
                    pairs.insert((value.clone(), req.clone()));
                }
            }
        }
        Self { pairs }
    }

    pub fn satisfies(&self, value: &Label, requirement: &Label) -> bool {
        self.pairs.contains(&(value.clone(), requirement.clone()))
    }
}

impl Default for TrustLattice {
    fn default() -> Self {
        Self::new()
    }
}
