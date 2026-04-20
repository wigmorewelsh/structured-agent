use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use nonempty::NonEmpty;

use crate::ast::{Definition, Module, ParsedModule};
use crate::types::FileId;

pub(crate) trait Discoverer {
    fn resolve(&self, path: &str) -> Result<String, String>;
}

pub(crate) struct FileDiscoverer;

impl Discoverer for FileDiscoverer {
    fn resolve(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
    }
}

pub(crate) struct InMemoryDiscoverer {
    sources: HashMap<String, String>,
}

impl InMemoryDiscoverer {
    pub(crate) fn new(sources: HashMap<String, String>) -> Self {
        Self { sources }
    }
}

impl Discoverer for InMemoryDiscoverer {
    fn resolve(&self, path: &str) -> Result<String, String> {
        self.sources
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Module not found: {}", path))
    }
}

fn to_file_path(entry_dir: &str, rel_path: &NonEmpty<String>) -> String {
    format!(
        "{}/{}.sa",
        entry_dir,
        rel_path.iter().cloned().collect::<Vec<_>>().join("/")
    )
}

fn to_resolve_key(rel_path: &NonEmpty<String>) -> String {
    rel_path.iter().cloned().collect::<Vec<_>>().join("/")
}

fn resolve_relative(current: &NonEmpty<String>, import: &NonEmpty<String>) -> NonEmpty<String> {
    let mut segments: Vec<String> = current.iter().cloned().collect();
    segments.pop();
    segments.extend(import.iter().cloned());
    NonEmpty::from_vec(segments).unwrap()
}

fn extract_inline_modules(
    module: &mut Module,
    parent_path: &NonEmpty<String>,
    file_id: crate::types::FileId,
) -> Vec<(NonEmpty<String>, Module)> {
    let mut result = Vec::new();
    let mut remaining = Vec::new();

    for def in module.definitions.drain(..) {
        if let Definition::InlineModule {
            name,
            definitions,
            span,
        } = def
        {
            let inline_path = {
                let mut p: Vec<String> = parent_path.iter().cloned().collect();
                p.push(name);
                NonEmpty::from_vec(p).unwrap()
            };
            let mut inline_mod = Module {
                definitions,
                span,
                file_id,
            };
            let nested = extract_inline_modules(&mut inline_mod, &inline_path, file_id);
            result.extend(nested);
            result.push((inline_path, inline_mod));
        } else {
            remaining.push(def);
        }
    }

    module.definitions = remaining;
    result
}

pub(crate) fn discover(
    entry_path: &str,
    entry_source: &str,
    discoverer: &impl Discoverer,
    native_module_names: &HashSet<String>,
    mut parse: impl FnMut(&str, &str) -> Result<(FileId, Module), String>,
) -> Result<Vec<ParsedModule>, String> {
    let entry_dir = Path::new(entry_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    let entry_stem = Path::new(entry_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string();

    let entry_rel = NonEmpty::new(entry_stem);

    let mut queue: VecDeque<(NonEmpty<String>, bool)> = VecDeque::new();
    queue.push_back((entry_rel, true));

    let mut visited: HashSet<NonEmpty<String>> = HashSet::new();
    let mut result: Vec<ParsedModule> = Vec::new();

    while let Some((rel_path, is_entry)) = queue.pop_front() {
        if !visited.insert(rel_path.clone()) {
            continue;
        }

        let source = if is_entry {
            entry_source.to_string()
        } else {
            let key = to_resolve_key(&rel_path);
            discoverer
                .resolve(&key)
                .or_else(|_| discoverer.resolve(&to_file_path(&entry_dir, &rel_path)))?
        };

        let parse_path = to_file_path(&entry_dir, &rel_path);
        let (file_id, mut module) = parse(&parse_path, &source)?;

        let inline_mods = extract_inline_modules(&mut module, &rel_path, file_id);

        for import in referenced_module_names(&module) {
            let dep_rel = match import {
                ImportType::Relative(path) => resolve_relative(&rel_path, &path),
                ImportType::Absolute(path) => path,
            };
            let dep_name = dep_rel.last().to_string();
            if !visited.contains(&dep_rel) && !native_module_names.contains(&dep_name) {
                queue.push_back((dep_rel, false));
            }
        }

        for (inline_rel, inline_module) in &inline_mods {
            for import in referenced_module_names(inline_module) {
                let dep_rel = match import {
                    ImportType::Relative(path) => resolve_relative(inline_rel, &path),
                    ImportType::Absolute(path) => path,
                };
                let dep_name = dep_rel.last().to_string();
                if !visited.contains(&dep_rel) && !native_module_names.contains(&dep_name) {
                    queue.push_back((dep_rel, false));
                }
            }
        }

        for (inline_rel, inline_module) in inline_mods {
            if visited.insert(inline_rel.clone()) {
                result.push(ParsedModule {
                    name: inline_rel,
                    file_id,
                    module: inline_module,
                    is_entry: false,
                    is_inline: true,
                });
            }
        }

        result.push(ParsedModule {
            name: rel_path,
            file_id,
            module,
            is_entry,
            is_inline: false,
        });
    }

    Ok(result)
}

#[derive(Debug, Clone)]
enum ImportType {
    Relative(NonEmpty<String>),
    Absolute(NonEmpty<String>),
}

fn referenced_module_names(module: &Module) -> Vec<ImportType> {
    let header_param_names: std::collections::HashSet<String> = module
        .definitions
        .iter()
        .flat_map(|def| {
            if let Definition::ModuleHeader { params, .. } = def {
                params.iter().map(|p| p.name.clone()).collect::<Vec<_>>()
            } else {
                vec![]
            }
        })
        .collect();

    module
        .definitions
        .iter()
        .flat_map(|def| match def {
            Definition::Use { path, .. } => {
                if header_param_names.contains(&path.first().name) {
                    return vec![];
                }
                let name_path = path.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
                let mut imports =
                    vec![ImportType::Relative(NonEmpty::from_vec(name_path).unwrap())];
                for seg in path.iter() {
                    for param in &seg.params {
                        let param_path = match param {
                            crate::ast::UseParam::Positional(p) => p.clone(),
                            crate::ast::UseParam::Named { path: p, .. } => p.clone(),
                        };
                        imports.push(ImportType::Relative(param_path));
                    }
                }
                imports
            }
            Definition::ModuleHeader { params, .. } => params
                .iter()
                .filter(|p| !p.path.is_empty())
                .map(|p| ImportType::Absolute(p.path.clone()))
                .collect(),
            _ => vec![],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Module, ModuleParam, UseSegment};
    use crate::types::Span;

    fn empty_module() -> Module {
        Module {
            definitions: vec![],
            span: Span::dummy(),
            file_id: 0,
        }
    }

    fn make_discoverer(entries: Vec<(&str, &str)>) -> InMemoryDiscoverer {
        InMemoryDiscoverer::new(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn discovers_entry_only() {
        let discoverer = make_discoverer(vec![]);
        let result = discover(
            "main.sa",
            "fn main(): () {}",
            &discoverer,
            &HashSet::new(),
            |_, _| Ok((0, empty_module())),
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, NonEmpty::new("main".to_string()));
        assert!(result[0].is_entry);
    }

    #[test]
    fn discovers_relative_use_dep() {
        let discoverer = make_discoverer(vec![("sample", "")]);
        let mut call_count = 0u32;
        let result = discover("main.sa", "", &discoverer, &HashSet::new(), |_, _| {
            let module = if call_count == 0 {
                call_count += 1;
                Module {
                    definitions: vec![Definition::Use {
                        path: NonEmpty::new(crate::ast::UseSegment {
                            name: "sample".to_string(),
                            params: vec![],
                            span: Span::dummy(),
                        }),
                        name: "Thing".to_string(),
                        alias: None,
                        is_pub: false,
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                }
            } else {
                empty_module()
            };
            Ok((0, module))
        })
        .unwrap();

        assert_eq!(result.len(), 2);
        let names: Vec<&NonEmpty<String>> = result.iter().map(|m| &m.name).collect();
        assert!(names.contains(&&NonEmpty::new("main".to_string())));
        assert!(names.contains(&&NonEmpty::new("sample".to_string())));
    }

    #[test]
    fn relative_use_from_submodule_resolves_within_subdir() {
        let discoverer =
            make_discoverer(vec![("submodule/other", ""), ("submodule/yetanother", "")]);
        let mut call_count = 0u32;
        let result = discover("root/main.sa", "", &discoverer, &HashSet::new(), |_, _| {
            let module = if call_count == 0 {
                call_count += 1;
                Module {
                    definitions: vec![Definition::Use {
                        path: nonempty::nonempty![
                            crate::ast::UseSegment {
                                name: "submodule".to_string(),
                                params: vec![],
                                span: Span::dummy(),
                            },
                            crate::ast::UseSegment {
                                name: "other".to_string(),
                                params: vec![],
                                span: Span::dummy(),
                            }
                        ],
                        name: "Thing".to_string(),
                        alias: None,
                        is_pub: false,
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                }
            } else if call_count == 1 {
                call_count += 1;
                Module {
                    definitions: vec![Definition::Use {
                        path: NonEmpty::new(crate::ast::UseSegment {
                            name: "yetanother".to_string(),
                            params: vec![],
                            span: Span::dummy(),
                        }),
                        name: "Thing".to_string(),
                        alias: None,
                        is_pub: false,
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                }
            } else {
                empty_module()
            };
            Ok((0, module))
        })
        .unwrap();

        assert_eq!(result.len(), 3);
        let names: Vec<&NonEmpty<String>> = result.iter().map(|m| &m.name).collect();
        assert!(names.contains(&&nonempty::nonempty![
            "submodule".to_string(),
            "other".to_string()
        ]));
        assert!(names.contains(&&nonempty::nonempty![
            "submodule".to_string(),
            "yetanother".to_string()
        ]));
    }

    #[test]
    fn module_header_absolute_param_resolves_from_root() {
        let discoverer = make_discoverer(vec![("sample", "")]);
        let mut call_count = 0u32;
        let result = discover(
            "root/submodule/other.sa",
            "",
            &discoverer,
            &HashSet::new(),
            |_, _| {
                let module = if call_count == 0 {
                    call_count += 1;
                    Module {
                        definitions: vec![Definition::ModuleHeader {
                            name: "other".to_string(),
                            params: vec![ModuleParam {
                                name: "sample".to_string(),
                                path: NonEmpty::new("sample".to_string()),
                                span: Span::dummy(),
                            }],
                            span: Span::dummy(),
                        }],
                        span: Span::dummy(),
                        file_id: 0,
                    }
                } else {
                    empty_module()
                };
                Ok((0, module))
            },
        )
        .unwrap();

        assert_eq!(result.len(), 2);
        let names: Vec<&NonEmpty<String>> = result.iter().map(|m| &m.name).collect();
        assert!(names.contains(&&NonEmpty::new("sample".to_string())));
    }

    #[test]
    fn skips_native_modules() {
        let discoverer = make_discoverer(vec![]);
        let mut native = HashSet::new();
        native.insert("io".to_string());

        let result = discover("main.sa", "", &discoverer, &native, |_, _| {
            Ok((
                0,
                Module {
                    definitions: vec![Definition::Use {
                        path: NonEmpty::new(crate::ast::UseSegment {
                            name: "io".to_string(),
                            params: vec![],
                            span: Span::dummy(),
                        }),
                        name: "print".to_string(),
                        alias: None,
                        is_pub: false,
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                },
            ))
        })
        .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn discovers_inline_module() {
        let discoverer = make_discoverer(vec![]);
        let result = discover("main.sa", "", &discoverer, &HashSet::new(), |_, _| {
            Ok((
                0,
                Module {
                    definitions: vec![Definition::InlineModule {
                        name: "math".to_string(),
                        definitions: vec![],
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                },
            ))
        })
        .unwrap();

        assert_eq!(result.len(), 2);
        let names: Vec<&NonEmpty<String>> = result.iter().map(|m| &m.name).collect();
        assert!(names.contains(&&NonEmpty::new("main".to_string())));
        assert!(names.contains(&&nonempty::nonempty![
            "main".to_string(),
            "math".to_string()
        ]));
    }

    #[test]
    fn inline_module_uses_are_discovered() {
        let discoverer = make_discoverer(vec![("main/other", "")]);
        let mut call_count = 0u32;
        let result = discover("main.sa", "", &discoverer, &HashSet::new(), |_, _| {
            let module = if call_count == 0 {
                call_count += 1;
                Module {
                    definitions: vec![Definition::InlineModule {
                        name: "math".to_string(),
                        definitions: vec![Definition::Use {
                            path: NonEmpty::new(UseSegment {
                                name: "other".to_string(),
                                params: vec![],
                                span: Span::dummy(),
                            }),
                            name: "Thing".to_string(),
                            alias: None,
                            is_pub: false,
                            span: Span::dummy(),
                        }],
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                }
            } else {
                empty_module()
            };
            Ok((0, module))
        })
        .unwrap();

        assert_eq!(result.len(), 3);
        let names: Vec<&NonEmpty<String>> = result.iter().map(|m| &m.name).collect();
        assert!(names.contains(&&nonempty::nonempty![
            "main".to_string(),
            "other".to_string()
        ]));
    }

    #[test]
    fn does_not_revisit_modules() {
        let discoverer = make_discoverer(vec![("sample", "")]);
        let mut call_count = 0u32;
        let result = discover("main.sa", "", &discoverer, &HashSet::new(), |_, _| {
            let module = if call_count < 2 {
                call_count += 1;
                Module {
                    definitions: vec![Definition::Use {
                        path: NonEmpty::new(crate::ast::UseSegment {
                            name: "sample".to_string(),
                            params: vec![],
                            span: Span::dummy(),
                        }),
                        name: "Thing".to_string(),
                        alias: None,
                        is_pub: false,
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                    file_id: 0,
                }
            } else {
                empty_module()
            };
            Ok((0, module))
        })
        .unwrap();

        assert_eq!(result.len(), 2);
    }
}
