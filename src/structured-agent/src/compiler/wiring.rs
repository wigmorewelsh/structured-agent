use std::collections::HashMap;

use crate::ast::Definition;

use super::discovery::ParsedModule;
use super::sigs::SigTable;

pub(crate) type Vtables = HashMap<String, HashMap<String, String>>;

pub(crate) fn resolve_vtables(modules: &[ParsedModule], sig_table: &SigTable) -> Vtables {
    let bindings = collect_bindings(modules);
    let wiring_sites = collect_wiring_sites(modules);
    let mut vtables: Vtables = HashMap::new();

    for parsed in modules {
        let params = match header_params(parsed) {
            Some(p) => p,
            None => continue,
        };

        let site_args = wiring_sites.get(&parsed.name);

        let mut vtable: HashMap<String, String> = HashMap::new();

        for (i, param) in params.iter().enumerate() {
            if param.path.len() < 2 {
                continue;
            }

            let concrete_module = site_args
                .and_then(|args| args.get(i))
                .and_then(|bound_name| bindings.get(bound_name))
                .map(|s| s.as_str())
                .unwrap_or(&param.path[0]);

            let sig_name = param.path.last().unwrap();

            let fn_names: Vec<String> = if let Some(fns) = sig_table.sig_definitions.get(sig_name) {
                fns.iter().map(|f| f.name.clone()).collect()
            } else {
                sig_table
                    .external_sigs
                    .keys()
                    .filter_map(|k| {
                        k.strip_prefix(&format!("{}::", concrete_module))
                            .map(str::to_string)
                    })
                    .collect()
            };

            for fn_name in fn_names {
                let param_key = format!("{}::{}", param.name, fn_name);
                let concrete_val = format!("{}::{}", concrete_module, fn_name);
                vtable.insert(param_key, concrete_val);
            }
        }

        if !vtable.is_empty() {
            vtables.insert(parsed.name.clone(), vtable);
        }
    }

    vtables
}

fn collect_bindings(modules: &[ParsedModule]) -> HashMap<String, String> {
    let mut bindings = HashMap::new();
    for parsed in modules {
        for def in &parsed.module.definitions {
            if let Definition::ModuleBinding {
                name, impl_path, ..
            } = def
            {
                if !impl_path.is_empty() {
                    bindings.insert(name.clone(), impl_path[0].clone());
                }
            }
        }
    }
    bindings
}

fn collect_wiring_sites(modules: &[ParsedModule]) -> HashMap<String, Vec<String>> {
    let mut sites: HashMap<String, Vec<String>> = HashMap::new();
    for parsed in modules {
        for def in &parsed.module.definitions {
            if let Definition::WiringSite { name, args, .. } = def {
                sites.insert(name.clone(), args.clone());
            }
        }
    }
    sites
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        Definition, Function, FunctionBody, ModuleParam, SigFunction, Type as AstType,
    };
    use crate::compiler::sigs::SigTable;
    use crate::types::{FileId, Span};

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
                    Definition::Function(Function {
                        name: "run".to_string(),
                        parameters: vec![],
                        return_type: AstType::Unit,
                        body: FunctionBody {
                            statements: vec![],
                            span: dummy_span(),
                        },
                        documentation: None,
                        is_pub: true,
                        span: dummy_span(),
                    }),
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
            parameters: vec![],
            return_type: AstType::Unit,
            span: dummy_span(),
        }
    }

    fn empty_sig_table() -> SigTable {
        SigTable {
            visibility: HashMap::new(),
            external_sigs: HashMap::new(),
            sig_definitions: HashMap::new(),
        }
    }

    #[test]
    fn test_no_params_produces_empty_vtables() {
        let parsed = make_module("tasks", vec![]);
        let vtables = resolve_vtables(&[parsed], &empty_sig_table());
        assert!(vtables.is_empty());
    }

    #[test]
    fn test_named_sig_builds_vtable_entries() {
        let p = param("io", &["storage", "Storage"]);
        let parsed = make_module("tasks", vec![p]);
        let mut table = empty_sig_table();
        table
            .sig_definitions
            .insert("Storage".to_string(), vec![sig_fn("read"), sig_fn("write")]);

        let vtables = resolve_vtables(&[parsed], &table);
        let vtable = vtables.get("tasks").expect("expected vtable for tasks");
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
        assert_eq!(vtable.get("io::write").unwrap(), "storage::write");
    }

    #[test]
    fn test_implicit_sig_from_module_exports() {
        let p = param("io", &["storage", "disk"]);
        let parsed = make_module("tasks", vec![p]);
        let mut table = empty_sig_table();
        table
            .external_sigs
            .insert("storage::read".to_string(), (vec![], AstType::Unit, true));
        table
            .external_sigs
            .insert("storage::write".to_string(), (vec![], AstType::Unit, true));

        let vtables = resolve_vtables(&[parsed], &table);
        let vtable = vtables.get("tasks").expect("expected vtable for tasks");
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
        assert_eq!(vtable.get("io::write").unwrap(), "storage::write");
    }

    #[test]
    fn test_multiple_params_builds_separate_entries() {
        let p1 = param("io", &["storage", "Storage"]);
        let p2 = param("log", &["logger", "Logger"]);
        let parsed = make_module("tasks", vec![p1, p2]);
        let mut table = empty_sig_table();
        table
            .sig_definitions
            .insert("Storage".to_string(), vec![sig_fn("read")]);
        table
            .sig_definitions
            .insert("Logger".to_string(), vec![sig_fn("write")]);

        let vtables = resolve_vtables(&[parsed], &table);
        let vtable = vtables.get("tasks").unwrap();
        assert_eq!(vtable.get("io::read").unwrap(), "storage::read");
        assert_eq!(vtable.get("log::write").unwrap(), "logger::write");
    }
}
