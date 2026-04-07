use std::collections::HashMap;

use crate::ast::Definition;
use crate::typecheck::checker::{CheckerRefs, FunctionKind};
use structured_agent_runtime::symbols::{
    FunctionName, FunctionNameKind, MetaData, ModuleName, TypeDefinitionKind,
};

use crate::ast::ParsedModule;

pub(crate) type Vtables = HashMap<String, HashMap<String, String>>;

pub(crate) fn resolve_vtables(
    modules: &[ParsedModule],
    metadata: &MetaData<CheckerRefs>,
) -> Vtables {
    let bindings = collect_bindings(modules);
    let wiring_sites = collect_wiring_sites(modules);

    modules
        .iter()
        .filter_map(|parsed| {
            let params = header_params(parsed)?;
            let site_args = wiring_sites.get(&parsed.name).map(Vec::as_slice);
            let vtable = build_vtable(params, site_args, &bindings, metadata);
            (!vtable.is_empty()).then(|| (parsed.name.clone(), vtable))
        })
        .collect()
}

fn build_vtable(
    params: &[crate::ast::ModuleParam],
    site_args: Option<&[String]>,
    bindings: &HashMap<String, String>,
    metadata: &MetaData<CheckerRefs>,
) -> HashMap<String, String> {
    params
        .iter()
        .enumerate()
        .filter(|(_, p)| p.path.len() >= 2)
        .flat_map(|(i, param)| {
            let concrete = concrete_module_for_param(i, param, site_args, bindings);
            let fn_names = fn_names_for_param(param, concrete, metadata);
            fn_names.into_iter().map(move |fn_name| {
                let param_key = format!("{}::{}", param.name, fn_name);
                let concrete_val = format!("{}::{}", concrete, fn_name);
                (param_key, concrete_val)
            })
        })
        .collect()
}

fn concrete_module_for_param<'a>(
    index: usize,
    param: &'a crate::ast::ModuleParam,
    site_args: Option<&'a [String]>,
    bindings: &'a HashMap<String, String>,
) -> &'a str {
    site_args
        .and_then(|args| args.get(index))
        .and_then(|bound_name| bindings.get(bound_name))
        .map(|s| s.as_str())
        .unwrap_or(&param.path[0])
}

fn fn_names_for_param(
    param: &crate::ast::ModuleParam,
    concrete_module: &str,
    metadata: &MetaData<CheckerRefs>,
) -> Vec<String> {
    let sig_name = param.path.last().unwrap();
    if let Some(type_def) = metadata.types.values().find(|td| {
        td.name.name == *sig_name && matches!(td.kind, TypeDefinitionKind::Signature { .. })
    }) {
        if let TypeDefinitionKind::Signature { entries } = &type_def.kind {
            return entries.iter().map(|e| e.name.clone()).collect();
        }
    }
    metadata
        .functions
        .keys()
        .filter_map(|fname| {
            if fname.module.to_string() == concrete_module {
                Some(fname.name.clone())
            } else {
                None
            }
        })
        .collect()
}

fn collect_bindings(modules: &[ParsedModule]) -> HashMap<String, String> {
    modules
        .iter()
        .flat_map(|parsed| &parsed.module.definitions)
        .filter_map(|def| {
            if let Definition::ModuleBinding {
                name, impl_path, ..
            } = def
            {
                impl_path.first().map(|m| (name.clone(), m.clone()))
            } else {
                None
            }
        })
        .collect()
}

fn collect_wiring_sites(modules: &[ParsedModule]) -> HashMap<String, Vec<String>> {
    modules
        .iter()
        .flat_map(|parsed| &parsed.module.definitions)
        .filter_map(|def| {
            if let Definition::WiringSite { name, args, .. } = def {
                Some((name.clone(), args.clone()))
            } else {
                None
            }
        })
        .collect()
}

fn header_params(parsed: &ParsedModule) -> Option<&Vec<crate::ast::ModuleParam>> {
    parsed.module.definitions.iter().find_map(|def| {
        if let Definition::ModuleHeader { params, .. } = def {
            Some(params)
        } else {
            None
        }
    })
}

pub(crate) fn lower_typed_module(
    module: &mut crate::typed_ast::Module,
    vtable: &std::collections::HashMap<String, String>,
    function_kinds: &std::collections::HashMap<String, FunctionKind>,
) {
    for def in &mut module.definitions {
        match def {
            crate::typed_ast::Definition::Function(f) => {
                lower_statements(&mut f.body.statements, vtable, function_kinds);
            }
            crate::typed_ast::Definition::TraitImpl { functions, .. } => {
                for f in functions {
                    lower_statements(&mut f.body.statements, vtable, function_kinds);
                }
            }
            _ => {}
        }
    }
}

fn lower_statements(
    stmts: &mut Vec<crate::typed_ast::Statement>,
    vtable: &std::collections::HashMap<String, String>,
    function_kinds: &std::collections::HashMap<String, FunctionKind>,
) {
    for stmt in stmts {
        match stmt {
            crate::typed_ast::Statement::Injection(e) => {
                lower_expression(e, vtable, function_kinds)
            }
            crate::typed_ast::Statement::Assignment { expression, .. } => {
                lower_expression(expression, vtable, function_kinds)
            }
            crate::typed_ast::Statement::VariableAssignment { expression, .. } => {
                lower_expression(expression, vtable, function_kinds)
            }
            crate::typed_ast::Statement::ExpressionStatement(e) => {
                lower_expression(e, vtable, function_kinds)
            }
            crate::typed_ast::Statement::Return(e) => lower_expression(e, vtable, function_kinds),
            crate::typed_ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                lower_expression(condition, vtable, function_kinds);
                lower_statements(body, vtable, function_kinds);
                if let Some(eb) = else_body {
                    lower_statements(eb, vtable, function_kinds);
                }
            }
            crate::typed_ast::Statement::While {
                condition, body, ..
            } => {
                lower_expression(condition, vtable, function_kinds);
                lower_statements(body, vtable, function_kinds);
            }
        }
    }
}

fn lower_expression(
    expr: &mut crate::typed_ast::Expression,
    vtable: &std::collections::HashMap<String, String>,
    function_kinds: &std::collections::HashMap<String, FunctionKind>,
) {
    match expr {
        crate::typed_ast::Expression::Call {
            resolved,
            kind,
            arguments,
            ..
        } => {
            let key = resolved.to_string();
            if let Some(concrete) = vtable.get(&key) {
                *resolved = match concrete.rsplit_once("::") {
                    Some((module, name)) => FunctionName {
                        name: name.to_string(),
                        module: ModuleName::from_str(module),
                        kind: FunctionNameKind::Function,
                    },
                    None => FunctionName {
                        name: concrete.to_string(),
                        module: ModuleName::from_str(""),
                        kind: FunctionNameKind::Function,
                    },
                };
                if let Some(new_kind) = function_kinds.get(concrete) {
                    *kind = new_kind.clone();
                }
            }
            for arg in arguments {
                lower_expression(arg, vtable, function_kinds);
            }
        }
        crate::typed_ast::Expression::StructLiteral { fields, .. } => {
            for (_, e) in fields {
                lower_expression(e, vtable, function_kinds);
            }
        }
        crate::typed_ast::Expression::FieldAccess { base, .. } => {
            lower_expression(base, vtable, function_kinds);
        }
        crate::typed_ast::Expression::ListLiteral { elements, .. } => {
            for e in elements {
                lower_expression(e, vtable, function_kinds);
            }
        }
        crate::typed_ast::Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            lower_expression(condition, vtable, function_kinds);
            lower_expression(then_expr, vtable, function_kinds);
            lower_expression(else_expr, vtable, function_kinds);
        }
        crate::typed_ast::Expression::Select(select, _) => {
            for clause in &mut select.clauses {
                lower_expression(&mut clause.expression_to_run, vtable, function_kinds);
                lower_expression(&mut clause.expression_next, vtable, function_kinds);
            }
        }
        crate::typed_ast::Expression::Variable { .. }
        | crate::typed_ast::Expression::StringLiteral { .. }
        | crate::typed_ast::Expression::BooleanLiteral { .. }
        | crate::typed_ast::Expression::IntLiteral { .. }
        | crate::typed_ast::Expression::Placeholder { .. }
        | crate::typed_ast::Expression::UnitLiteral { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::AstSignature;
    use crate::ast::{
        Definition, Function, FunctionBody, ModuleParam, SigFunction, Type as AstType,
    };
    use crate::typecheck::checker::{CheckerAstRef, CheckerRefs, FunctionKind, SourceLocation};
    use crate::types::{FileId, Span};
    use std::sync::Arc;
    use structured_agent_runtime::symbols::{
        FunctionDefinition, FunctionName, FunctionNameKind, MetaData, ModuleName, SignatureEntry,
        TypeDefinition, TypeDefinitionKind, TypeName,
    };

    fn dummy_span() -> Span {
        Span::dummy()
    }

    fn file_id() -> FileId {
        0
    }

    fn make_module(name: &str, params: Vec<ModuleParam>) -> ParsedModule {
        ParsedModule {
            name: name.to_string(),
            module: crate::ast::Module {
                definitions: vec![
                    Definition::ModuleHeader {
                        name: name.to_string(),
                        params,
                        span: dummy_span(),
                    },
                    Definition::Function(Arc::new(Function {
                        name: "run".to_string(),
                        type_params: vec![],
                        parameters: vec![],
                        return_type: AstType::Unit,
                        body: FunctionBody {
                            statements: vec![],
                            span: dummy_span(),
                        },
                        documentation: None,
                        is_pub: true,
                        span: dummy_span(),
                    })),
                ],
                span: dummy_span(),
                file_id: file_id(),
            },
            is_entry: false,
            file_id: file_id(),
        }
    }

    fn param(name: &str, path: &[&str]) -> ModuleParam {
        ModuleParam {
            name: name.to_string(),
            path: path.iter().map(|s| s.to_string()).collect(),
            span: dummy_span(),
        }
    }

    fn sig_fn(name: &str) -> SigFunction {
        SigFunction {
            name: name.to_string(),
            type_params: vec![],
            parameters: vec![],
            return_type: AstType::Unit,
            span: dummy_span(),
        }
    }

    fn add_sig(metadata: &mut MetaData<CheckerRefs>, sig_name: &str, fns: &[SigFunction]) {
        let type_name = TypeName {
            name: sig_name.to_string(),
            module: ModuleName::from_str("test"),
        };
        let entry = TypeDefinition {
            name: type_name.clone(),
            kind: TypeDefinitionKind::Signature {
                entries: fns
                    .iter()
                    .map(|f| SignatureEntry {
                        name: f.name.clone(),
                        type_name: TypeName {
                            name: "unit".to_string(),
                            module: ModuleName::from_str(""),
                        },
                    })
                    .collect(),
            },
            source_ref: SourceLocation(0, Span::dummy()),
            ast_ref: CheckerAstRef::Signature(Arc::new(AstSignature {
                name: sig_name.to_string(),
                functions: fns.to_vec(),
                span: Span::dummy(),
            })),
        };
        metadata.types.insert(type_name, Arc::new(entry));
    }

    fn add_fn(metadata: &mut MetaData<CheckerRefs>, module: &str, fn_name: &str) {
        let name = FunctionName {
            name: fn_name.to_string(),
            module: ModuleName::from_str(module),
            kind: FunctionNameKind::Function,
        };
        let fdef = FunctionDefinition {
            name: name.clone(),
            type_name: TypeName {
                name: "unit".to_string(),
                module: ModuleName::from_str(module),
            },
            source_ref: SourceLocation(0, Span::dummy()),
            ast_ref: CheckerAstRef::ExternalFn {
                params: vec![],
                return_type: AstType::Unit,
                type_params: vec![],
                kind: FunctionKind::External,
            },
            body_ref: None,
        };
        metadata.functions.insert(name, Arc::new(fdef));
    }

    #[test]
    fn test_no_params_produces_empty_vtables() {
        let parsed = make_module("tasks", vec![]);
        let vtables = resolve_vtables(&[parsed], &MetaData::default());
        assert!(vtables.is_empty());
    }

    #[test]
    fn test_named_sig_builds_vtable_entries() {
        let p = param("io", &["storage", "Storage"]);
        let parsed = make_module("tasks", vec![p]);
        let mut metadata = MetaData::default();
        add_sig(&mut metadata, "Storage", &[sig_fn("read"), sig_fn("write")]);

        let vtables = resolve_vtables(&[parsed], &metadata);
        let vtable = vtables.get("tasks").expect("expected vtable for tasks");
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
        assert_eq!(vtable.get("io::write").unwrap(), "storage::write");
    }

    #[test]
    fn test_implicit_sig_from_module_exports() {
        let p = param("io", &["storage", "disk"]);
        let parsed = make_module("tasks", vec![p]);
        let mut metadata = MetaData::default();
        add_fn(&mut metadata, "storage", "read");
        add_fn(&mut metadata, "storage", "write");

        let vtables = resolve_vtables(&[parsed], &metadata);
        let vtable = vtables.get("tasks").expect("expected vtable for tasks");
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
        assert_eq!(vtable.get("io::write").unwrap(), "storage::write");
    }

    #[test]
    fn test_multiple_params_builds_separate_entries() {
        let p1 = param("io", &["storage", "Storage"]);
        let p2 = param("log", &["logger", "Logger"]);
        let parsed = make_module("tasks", vec![p1, p2]);
        let mut metadata = MetaData::default();
        add_sig(&mut metadata, "Storage", &[sig_fn("read")]);
        add_sig(&mut metadata, "Logger", &[sig_fn("write")]);

        let vtables = resolve_vtables(&[parsed], &metadata);
        let vtable = vtables.get("tasks").unwrap();
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
        assert_eq!(vtable.get("log::write").unwrap(), "logger::write");
    }

    fn make_entry_module(definitions: Vec<Definition>) -> ParsedModule {
        ParsedModule {
            name: "main".to_string(),
            module: crate::ast::Module {
                definitions,
                span: dummy_span(),
                file_id: file_id(),
            },
            is_entry: true,
            file_id: file_id(),
        }
    }

    fn binding(name: &str, sig_path: &[&str], impl_path: &[&str]) -> Definition {
        Definition::ModuleBinding {
            name: name.to_string(),
            sig_path: sig_path.iter().map(|s| s.to_string()).collect(),
            impl_path: impl_path.iter().map(|s| s.to_string()).collect(),
            span: dummy_span(),
        }
    }

    fn wiring_site(module_name: &str, args: &[&str]) -> Definition {
        Definition::WiringSite {
            name: module_name.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            span: dummy_span(),
        }
    }

    #[test]
    fn test_explicit_binding_overrides_inferred_concrete_module() {
        let p = param("io", &["real_store", "Store"]);
        let tasks = make_module("tasks", vec![p]);
        let entry = make_entry_module(vec![
            binding("io", &["real_store", "Store"], &["mock_store"]),
            wiring_site("tasks", &["io"]),
        ]);

        let mut metadata = MetaData::default();
        add_sig(&mut metadata, "Store", &[sig_fn("read")]);

        let vtables = resolve_vtables(&[tasks, entry], &metadata);
        let vtable = vtables.get("tasks").expect("expected vtable for tasks");
        assert_eq!(
            vtable.get("io::read").unwrap(),
            "mock_store::read",
            "vtable should use the bound impl, not the param path"
        );
    }

    #[test]
    fn test_wiring_site_without_binding_falls_back_to_param_path() {
        let p = param("io", &["storage", "Store"]);
        let tasks = make_module("tasks", vec![p]);
        let entry = make_entry_module(vec![wiring_site("tasks", &["io"])]);

        let mut metadata = MetaData::default();
        add_sig(&mut metadata, "Store", &[sig_fn("read")]);

        let vtables = resolve_vtables(&[tasks, entry], &metadata);
        let vtable = vtables.get("tasks").expect("expected vtable for tasks");
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
    }

    #[test]
    fn test_collect_bindings_extracts_impl_path() {
        let entry = make_entry_module(vec![binding(
            "fmt",
            &["formatter", "Formatter"],
            &["mock_formatter"],
        )]);
        let bindings = collect_bindings(&[entry]);
        assert_eq!(
            bindings.get("fmt").map(String::as_str),
            Some("mock_formatter")
        );
    }

    #[test]
    fn test_collect_wiring_sites_extracts_args() {
        let entry = make_entry_module(vec![wiring_site("reporter", &["fmt"])]);
        let sites = collect_wiring_sites(&[entry]);
        assert_eq!(
            sites.get("reporter").map(Vec::as_slice),
            Some(vec!["fmt".to_string()].as_slice())
        );
    }

    #[test]
    fn test_module_without_header_produces_no_vtable_entry() {
        let entry = make_entry_module(vec![
            binding("io", &["storage", "Store"], &["mock_store"]),
            wiring_site("tasks", &["io"]),
        ]);
        let vtables = resolve_vtables(&[entry], &MetaData::default());
        assert!(vtables.is_empty());
    }

    #[test]
    fn test_param_with_single_segment_path_skipped() {
        let p = param("io", &["storage"]);
        let parsed = make_module("tasks", vec![p]);
        let vtables = resolve_vtables(&[parsed], &MetaData::default());
        assert!(vtables.is_empty());
    }

    #[test]
    fn test_lower_typed_module_substitutes_call() {
        let span = Span::dummy();
        let call = crate::typed_ast::Expression::Call {
            function: "io::read".to_string(),
            resolved: FunctionName {
                name: "read".to_string(),
                module: ModuleName::from_str("io"),
                kind: FunctionNameKind::Function,
            },
            kind: FunctionKind::External,
            arguments: vec![],
            ty: AstType::String,
            span,
        };
        let mut module = crate::typed_ast::Module {
            definitions: vec![crate::typed_ast::Definition::Function(
                crate::typed_ast::Function {
                    name: "run".to_string(),
                    parameters: vec![],
                    return_type: AstType::Unit,
                    body: crate::typed_ast::FunctionBody {
                        statements: vec![crate::typed_ast::Statement::Return(call)],
                        span,
                    },
                    documentation: None,
                    is_pub: true,
                    span,
                },
            )],
            span,
            file_id: 0,
        };

        let mut vtable = std::collections::HashMap::new();
        vtable.insert("io::read".to_string(), "storage::read".to_string());

        lower_typed_module(&mut module, &vtable, &HashMap::new());

        if let crate::typed_ast::Definition::Function(f) = &module.definitions[0] {
            if let crate::typed_ast::Statement::Return(crate::typed_ast::Expression::Call {
                resolved,
                ..
            }) = &f.body.statements[0]
            {
                assert_eq!(resolved.to_string(), "storage::read");
            } else {
                panic!("expected return with call");
            }
        } else {
            panic!("expected function definition");
        }
    }
}
