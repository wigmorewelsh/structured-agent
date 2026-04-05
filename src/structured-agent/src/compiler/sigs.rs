use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{Definition, SigFunction};
use crate::typecheck::checker::{ExternalSig, FunctionKind, ModuleVisibility};
use crate::types::Span;
use structured_agent_runtime::types::Module as RuntimeModule;

use super::discovery::ParsedModule;

#[derive(Debug, Clone)]
pub(crate) struct SigTable {
    pub(crate) visibility: ModuleVisibility,
    pub(crate) external_sigs: HashMap<String, ExternalSig>,
    pub(crate) sig_definitions: HashMap<String, Vec<SigFunction>>,
}

pub(crate) fn collect_sigs(
    modules: &[ParsedModule],
    native_modules: &HashMap<String, Arc<dyn RuntimeModule>>,
) -> SigTable {
    let mut visibility: ModuleVisibility = HashMap::new();
    let mut external_sigs: HashMap<String, ExternalSig> = HashMap::new();
    let mut sig_definitions: HashMap<String, Vec<SigFunction>> = HashMap::new();

    for parsed in modules {
        for def in &parsed.module.definitions {
            match def {
                Definition::Function(f) => {
                    let qname = qualified(&parsed.name, &f.name, parsed.is_entry);
                    visibility.insert(qname.clone(), f.is_pub);
                    if !parsed.is_entry {
                        external_sigs.insert(
                            qname,
                            ExternalSig::new(
                                f.parameters.clone(),
                                f.return_type.clone(),
                                f.is_pub,
                                FunctionKind::Bytecode,
                            ),
                        );
                    }
                }
                Definition::ExternalFunction(f) => {
                    let qname = qualified(&parsed.name, &f.name, parsed.is_entry);
                    visibility.insert(qname.clone(), f.is_pub);
                    if !parsed.is_entry {
                        external_sigs.insert(
                            qname,
                            ExternalSig::new(
                                f.parameters.clone(),
                                f.return_type.clone(),
                                f.is_pub,
                                FunctionKind::External,
                            ),
                        );
                    }
                }
                Definition::Use { path, .. } => {
                    if path.len() < 2 {
                        continue;
                    }
                    let module_name = &path[0];
                    let fn_name = path.last().unwrap();
                    let Some(module) = native_modules.get(module_name) else {
                        continue;
                    };
                    let Some(func) = module.functions().into_iter().find(|f| f.name() == fn_name)
                    else {
                        continue;
                    };
                    let qname = format!("{}::{}", module_name, fn_name);
                    let parameters: Vec<crate::ast::Parameter> = func
                        .parameters()
                        .iter()
                        .map(|p| crate::ast::Parameter {
                            name: p.name.clone(),
                            param_type: super::runtime_type_to_ast(&p.param_type),
                            span: Span::dummy(),
                        })
                        .collect();
                    let return_type = super::runtime_type_to_ast(func.return_type());
                    visibility.insert(qname.clone(), true);
                    external_sigs.insert(
                        qname,
                        ExternalSig::new(parameters, return_type, true, FunctionKind::External)
                            .with_type_params(
                                func.type_params()
                                    .iter()
                                    .map(|s| crate::ast::TypeParam::from(s.as_str()))
                                    .collect(),
                            ),
                    );
                }
                Definition::Signature {
                    name, functions, ..
                } => {
                    sig_definitions.insert(name.clone(), functions.clone());
                }
                _ => {}
            }
        }
    }

    SigTable {
        visibility,
        external_sigs,
        sig_definitions,
    }
}

pub(crate) fn sigs_visible_to_module(
    module: &crate::ast::Module,
    sig_table: &SigTable,
) -> HashMap<String, ExternalSig> {
    module
        .definitions
        .iter()
        .filter_map(|def| {
            let Definition::Use { path, alias, .. } = def else {
                return None;
            };
            if path.len() < 2 {
                return None;
            }
            let qualified = format!("{}::{}", path[0], path.last().unwrap());
            let sig = sig_table.external_sigs.get(&qualified)?;
            let key = alias
                .clone()
                .unwrap_or_else(|| path.last().unwrap().clone());
            Some((key, sig.clone()))
        })
        .collect()
}

fn qualified(module: &str, function: &str, is_entry: bool) -> String {
    if is_entry {
        function.to_string()
    } else {
        format!("{}::{}", module, function)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        Definition, ExternalFunction, Function, FunctionBody, Module, ModuleParam, Parameter,
        SigFunction, Type as AstType,
    };
    use crate::types::{FileId, Span};

    fn dummy_span() -> Span {
        Span::dummy()
    }

    fn make_parsed(name: &str, is_entry: bool, definitions: Vec<Definition>) -> ParsedModule {
        ParsedModule {
            name: name.to_string(),
            module: Module {
                definitions,
                span: dummy_span(),
                file_id: 0,
            },
            is_entry,
            file_id: 0,
        }
    }

    fn pub_fn(name: &str) -> Definition {
        Definition::Function(Function {
            name: name.to_string(),
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
        })
    }

    fn pub_ext_fn(name: &str) -> Definition {
        Definition::ExternalFunction(ExternalFunction {
            name: name.to_string(),
            type_params: vec![],
            parameters: vec![],
            return_type: AstType::Unit,
            is_pub: true,
            span: dummy_span(),
        })
    }

    fn sig_def(name: &str, fn_names: &[&str]) -> Definition {
        Definition::Signature {
            name: name.to_string(),
            functions: fn_names
                .iter()
                .map(|n| SigFunction {
                    name: n.to_string(),
                    type_params: vec![],
                    parameters: vec![],
                    return_type: AstType::Unit,
                    span: dummy_span(),
                })
                .collect(),
            span: dummy_span(),
        }
    }

    #[test]
    fn test_entry_module_fns_in_visibility_not_external_sigs() {
        let modules = vec![make_parsed("main", true, vec![pub_fn("run")])];
        let table = collect_sigs(&modules, &HashMap::new());
        assert!(table.visibility.contains_key("run"));
        assert!(!table.external_sigs.contains_key("run"));
    }

    #[test]
    fn test_non_entry_module_fns_in_both_visibility_and_external_sigs() {
        let modules = vec![make_parsed("lib", false, vec![pub_fn("read")])];
        let table = collect_sigs(&modules, &HashMap::new());
        assert!(table.visibility.contains_key("lib::read"));
        assert!(table.external_sigs.contains_key("lib::read"));
    }

    #[test]
    fn test_external_function_collected_in_external_sigs() {
        let modules = vec![make_parsed("lib", false, vec![pub_ext_fn("fetch")])];
        let table = collect_sigs(&modules, &HashMap::new());
        assert!(table.external_sigs.contains_key("lib::fetch"));
    }

    #[test]
    fn test_signature_definition_collected() {
        let modules = vec![make_parsed(
            "lib",
            false,
            vec![sig_def("Store", &["read", "write"])],
        )];
        let table = collect_sigs(&modules, &HashMap::new());
        let fns = table
            .sig_definitions
            .get("Store")
            .expect("Store sig missing");
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "read");
        assert_eq!(fns[1].name, "write");
    }

    #[test]
    fn test_qualified_name_is_module_prefixed_for_non_entry() {
        let modules = vec![make_parsed("mymod", false, vec![pub_fn("greet")])];
        let table = collect_sigs(&modules, &HashMap::new());
        assert!(table.external_sigs.contains_key("mymod::greet"));
        assert!(!table.external_sigs.contains_key("greet"));
    }

    #[test]
    fn test_sigs_visible_to_module_resolves_use_alias() {
        let mut table = SigTable {
            visibility: HashMap::new(),
            external_sigs: HashMap::new(),
            sig_definitions: HashMap::new(),
        };
        table.external_sigs.insert(
            "storage::read".to_string(),
            ExternalSig::new(vec![], AstType::Unit, true, FunctionKind::External),
        );

        let module = Module {
            definitions: vec![Definition::Use {
                path: vec!["storage".to_string(), "read".to_string()],
                alias: None,
                is_pub: false,
                span: dummy_span(),
            }],
            span: dummy_span(),
            file_id: 0,
        };

        let visible = sigs_visible_to_module(&module, &table);
        assert!(
            visible.contains_key("read"),
            "expected 'read' in visible sigs"
        );
    }

    #[test]
    fn test_sigs_visible_to_module_with_alias() {
        let mut table = SigTable {
            visibility: HashMap::new(),
            external_sigs: HashMap::new(),
            sig_definitions: HashMap::new(),
        };
        table.external_sigs.insert(
            "storage::read".to_string(),
            ExternalSig::new(vec![], AstType::Unit, true, FunctionKind::External),
        );

        let module = Module {
            definitions: vec![Definition::Use {
                path: vec!["storage".to_string(), "read".to_string()],
                alias: Some("fetch".to_string()),
                is_pub: false,
                span: dummy_span(),
            }],
            span: dummy_span(),
            file_id: 0,
        };

        let visible = sigs_visible_to_module(&module, &table);
        assert!(
            visible.contains_key("fetch"),
            "expected alias 'fetch' in visible sigs"
        );
        assert!(!visible.contains_key("read"));
    }

    #[test]
    fn test_sigs_visible_resolves_three_segment_use() {
        let mut table = SigTable {
            visibility: HashMap::new(),
            external_sigs: HashMap::new(),
            sig_definitions: HashMap::new(),
        };
        table.external_sigs.insert(
            "foo::baz".to_string(),
            ExternalSig::new(vec![], AstType::Unit, true, FunctionKind::External),
        );

        let module = Module {
            definitions: vec![Definition::Use {
                path: vec!["foo".to_string(), "bar".to_string(), "baz".to_string()],
                alias: None,
                is_pub: false,
                span: dummy_span(),
            }],
            span: dummy_span(),
            file_id: 0,
        };

        let visible = sigs_visible_to_module(&module, &table);
        assert!(
            visible.contains_key("baz"),
            "three-segment use path must resolve"
        );
    }

    #[test]
    fn test_sigs_visible_resolves_two_segment_use() {
        let mut table = SigTable {
            visibility: HashMap::new(),
            external_sigs: HashMap::new(),
            sig_definitions: HashMap::new(),
        };
        table.external_sigs.insert(
            "storage::read".to_string(),
            ExternalSig::new(vec![], AstType::Unit, true, FunctionKind::External),
        );

        let module = Module {
            definitions: vec![Definition::Use {
                path: vec!["storage".to_string(), "read".to_string()],
                alias: None,
                is_pub: false,
                span: dummy_span(),
            }],
            span: dummy_span(),
            file_id: 0,
        };

        let visible = sigs_visible_to_module(&module, &table);
        assert!(
            visible.contains_key("read"),
            "two-segment use path must resolve"
        );
    }

    #[test]
    fn test_sigs_visible_ignores_single_segment_use() {
        let mut table = SigTable {
            visibility: HashMap::new(),
            external_sigs: HashMap::new(),
            sig_definitions: HashMap::new(),
        };
        table.external_sigs.insert(
            "read".to_string(),
            ExternalSig::new(vec![], AstType::Unit, true, FunctionKind::External),
        );

        let module = Module {
            definitions: vec![Definition::Use {
                path: vec!["read".to_string()],
                alias: None,
                is_pub: false,
                span: dummy_span(),
            }],
            span: dummy_span(),
            file_id: 0,
        };

        let visible = sigs_visible_to_module(&module, &table);
        assert!(visible.is_empty());
    }
}
