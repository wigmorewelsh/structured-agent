// The trust lattice order is derived from the declaration order of this enum via PartialOrd.
// satisfies(value, requirement) is then just value >= requirement with no special casing.
// This only works because the variants are declared lowest-trust first. If the order
// were ever changed or a new level inserted in the middle, the lattice semantics would
// silently break.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Label {
    Untrusted,
    External,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BaseType {
    Int,
    Str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LabelExpr {
    Concrete(Label),
    Var(String),
}
