use std::collections::HashMap;

use crate::ast::Definition;
use crate::typecheck::checker::{FunctionSignatureTuple, ModuleVisibility};

use super::discovery::ParsedModule;

#[derive(Debug, Clone)]
pub(crate) struct SigTable {
    pub(crate) visibility: ModuleVisibility,
    pub(crate) external_sigs: HashMap<String, FunctionSignatureTuple>,
}

pub(crate) fn collect_sigs(modules: &[ParsedModule]) -> SigTable {
    let mut visibility: ModuleVisibility = HashMap::new();
    let mut external_sigs: HashMap<String, FunctionSignatureTuple> = HashMap::new();

    for parsed in modules {
        for def in &parsed.module.definitions {
            match def {
                Definition::Function(f) => {
                    let qname = qualified(&parsed.name, &f.name, parsed.is_entry);
                    visibility.insert(qname.clone(), f.is_pub);
                    if !parsed.is_entry {
                        external_sigs.insert(
                            qname,
                            (f.parameters.clone(), f.return_type.clone(), f.is_pub),
                        );
                    }
                }
                Definition::ExternalFunction(f) => {
                    let qname = qualified(&parsed.name, &f.name, parsed.is_entry);
                    visibility.insert(qname.clone(), f.is_pub);
                    if !parsed.is_entry {
                        external_sigs.insert(
                            qname,
                            (f.parameters.clone(), f.return_type.clone(), f.is_pub),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    SigTable {
        visibility,
        external_sigs,
    }
}

pub(crate) fn sigs_visible_to_module(
    module: &crate::ast::Module,
    sig_table: &SigTable,
) -> HashMap<String, FunctionSignatureTuple> {
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
            let qualified = format!("{}.{}", path[0], path.last().unwrap());
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
        format!("{}.{}", module, function)
    }
}
