use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::ast::{Definition, Module};
use crate::types::FileId;

#[derive(Debug)]
pub(crate) struct ParsedModule {
    pub(crate) name: String,
    pub(crate) module: Module,
    pub(crate) is_entry: bool,
    pub(crate) file_id: FileId,
}

pub(crate) trait Discoverer {
    fn resolve(&self, path: &str) -> Result<String, String>;
    fn dep_path(&self, entry_dir: &str, module_name: &str) -> String;
}

pub(crate) struct FileDiscoverer;

impl Discoverer for FileDiscoverer {
    fn resolve(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
    }

    fn dep_path(&self, entry_dir: &str, module_name: &str) -> String {
        format!("{}/{}.sa", entry_dir, module_name)
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

    fn dep_path(&self, _entry_dir: &str, module_name: &str) -> String {
        module_name.to_string()
    }
}

pub(crate) fn discover(
    entry_path: &str,
    entry_source: &str,
    discoverer: &impl Discoverer,
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

    let mut queue: VecDeque<(String, String, bool)> = VecDeque::new();
    queue.push_back((entry_stem, entry_path.to_string(), true));

    let mut visited: HashSet<String> = HashSet::new();
    let mut result: Vec<ParsedModule> = Vec::new();

    while let Some((name, file_path, is_entry)) = queue.pop_front() {
        if !visited.insert(name.clone()) {
            continue;
        }

        let source = if is_entry {
            entry_source.to_string()
        } else {
            discoverer.resolve(&file_path)?
        };

        let (file_id, module) = parse(&file_path, &source)?;

        for dep in referenced_module_names(&module) {
            if !visited.contains(&dep) {
                let dep_path = discoverer.dep_path(&entry_dir, &dep);
                queue.push_back((dep.clone(), dep_path, false));
            }
        }

        result.push(ParsedModule {
            name,
            file_id,
            module,
            is_entry,
        });
    }

    Ok(result)
}

pub(crate) fn referenced_module_names(module: &Module) -> Vec<String> {
    module
        .definitions
        .iter()
        .flat_map(|def| match def {
            Definition::Use { path, .. } if path.len() > 1 => vec![path[0].clone()],
            Definition::ModuleHeader { params, .. } => params
                .iter()
                .filter(|p| !p.path.is_empty())
                .map(|p| p.path[0].clone())
                .collect(),
            Definition::ModuleBinding { impl_path, .. } if !impl_path.is_empty() => {
                vec![impl_path[0].clone()]
            }
            _ => vec![],
        })
        .collect()
}
