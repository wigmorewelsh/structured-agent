use label_solver::{
    constraint::ConstraintKind,
    solver::Solver,
    types::{Label, LabelExpr},
};

#[test]
fn test_fetch_to_parse_succeeds() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelFlow {
        from: LabelExpr::Concrete(Label::Untrusted),
        to: LabelExpr::Concrete(Label::Untrusted),
    });
    solver.solve();
    assert!(!solver.has_errors());
}

#[test]
fn test_fetch_to_log_fails() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelFlow {
        from: LabelExpr::Concrete(Label::Untrusted),
        to: LabelExpr::Concrete(Label::Internal),
    });
    solver.solve();
    assert!(solver.has_errors());
    assert_eq!(solver.errors.len(), 1);
}

#[test]
fn test_identity_with_untrusted() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelUnify {
        var: "l".to_string(),
        label: Label::Untrusted,
    });
    solver.solve();
    assert!(!solver.has_errors());
    assert_eq!(solver.substitution.get("l"), Some(&Label::Untrusted));
}

#[test]
fn test_identity_with_internal() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelUnify {
        var: "l".to_string(),
        label: Label::Internal,
    });
    solver.solve();
    assert!(!solver.has_errors());
    assert_eq!(solver.substitution.get("l"), Some(&Label::Internal));
}

#[test]
fn test_conflicting_label_unify() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelUnify {
        var: "x".to_string(),
        label: Label::Untrusted,
    });
    solver.add_wanted(ConstraintKind::LabelUnify {
        var: "x".to_string(),
        label: Label::Internal,
    });
    solver.solve();
    assert!(solver.has_errors());
    assert_eq!(solver.errors.len(), 1);
}

#[test]
fn test_derived_drives_further_wanted() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelFlow {
        from: LabelExpr::Var("l".to_string()),
        to: LabelExpr::Concrete(Label::Untrusted),
    });
    solver.add_wanted(ConstraintKind::LabelUnify {
        var: "l".to_string(),
        label: Label::Untrusted,
    });
    solver.solve();
    assert!(!solver.has_errors());
    assert_eq!(solver.substitution.get("l"), Some(&Label::Untrusted));
    assert_eq!(solver.inert.len(), 1);
}

#[test]
fn test_lattice_internal_satisfies_external() {
    let mut solver = Solver::new();
    solver.add_wanted(ConstraintKind::LabelFlow {
        from: LabelExpr::Concrete(Label::Internal),
        to: LabelExpr::Concrete(Label::External),
    });
    solver.solve();
    assert!(!solver.has_errors());
}
