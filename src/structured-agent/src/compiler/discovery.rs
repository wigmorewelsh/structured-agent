use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use combine::Parser as CombineParser;
use combine::stream::{easy, position};
use dashmap::DashMap;
use nonempty::NonEmpty;

use crate::ast::{Definition, Module, ParsedModule, UseParam};
use crate::compiler::parser;
use crate::types::{FileId, SourceFiles};

pub(crate) trait Discoverer: Send + Sync {
    fn resolve(&self, path: &str) -> Result<String, String>;
}

pub(crate) struct FileDiscoverer;

impl Discoverer for FileDiscoverer {
    fn resolve(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))
    }
}

pub(crate) struct InMemoryDiscoverer {
    sources: std::collections::HashMap<String, String>,
}

impl InMemoryDiscoverer {
    pub(crate) fn new(sources: std::collections::HashMap<String, String>) -> Self {
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

pub(crate) struct DiscoveredModule {
    pub(crate) name: NonEmpty<String>,
    pub(crate) module: Module,
    pub(crate) is_entry: bool,
    pub(crate) file_id: FileId,
    pub(crate) is_inline: bool,
    pub(crate) source_info: Option<(String, String)>,
}

impl DiscoveredModule {
    pub(crate) fn into_parsed(self) -> ParsedModule {
        ParsedModule {
            name: self.name,
            module: self.module,
            is_entry: self.is_entry,
            file_id: self.file_id,
            is_inline: self.is_inline,
        }
    }
}

#[salsa::input]
struct ModuleSource {
    logical: Vec<String>,
    contents: String,
    base: Vec<String>,
    file_id: FileId,
    display_name: String,
}

#[salsa::db]
pub(crate) trait DiscoveryDatabase: salsa::Database {
    fn input(&self, logical: Vec<String>) -> Option<ModuleSource>;
}

#[salsa::db]
#[derive(Clone)]
pub(crate) struct DiscoveryDb {
    storage: salsa::Storage<Self>,
    sources: Arc<DashMap<Vec<String>, ModuleSource>>,
    entry_dir: Arc<String>,
    native_modules: Arc<HashSet<String>>,
    discoverer: Arc<dyn Discoverer>,
    source_files: SourceFiles,
}

impl DiscoveryDb {
    fn new(
        entry_dir: String,
        native_modules: HashSet<String>,
        discoverer: Arc<dyn Discoverer>,
        source_files: SourceFiles,
    ) -> Self {
        Self {
            storage: Default::default(),
            sources: Arc::new(DashMap::new()),
            entry_dir: Arc::new(entry_dir),
            native_modules: Arc::new(native_modules),
            discoverer,
            source_files,
        }
    }
}

#[salsa::db]
impl salsa::Database for DiscoveryDb {}

#[salsa::db]
impl DiscoveryDatabase for DiscoveryDb {
    fn input(&self, logical: Vec<String>) -> Option<ModuleSource> {
        if logical.iter().any(|s| s.is_empty()) {
            return None;
        }
        if self.native_modules.contains(logical.last()?) {
            return None;
        }
        match self.sources.entry(logical.clone()) {
            dashmap::Entry::Occupied(e) => Some(*e.get()),
            dashmap::Entry::Vacant(e) => {
                let key = logical.join("/");

                let try_raw = self.discoverer.resolve(&key).ok().map(|s| {
                    let mut base = logical.clone();
                    base.pop();
                    (s, base, key.clone())
                });

                let try_file = try_raw.or_else(|| {
                    let file_path = format!("{}/{}.sa", self.entry_dir, key);
                    self.discoverer.resolve(&file_path).ok().map(|s| {
                        let mut base = logical.clone();
                        base.pop();
                        (s, base, file_path)
                    })
                });

                let resolved = try_file.or_else(|| {
                    let mod_path = format!("{}/{}/mod.sa", self.entry_dir, key);
                    self.discoverer
                        .resolve(&mod_path)
                        .ok()
                        .map(|s| (s, logical.clone(), mod_path))
                })?;

                let (source, base, display_name) = resolved;
                let file_id = self.source_files.add(display_name.clone(), source.clone());
                let ms = ModuleSource::new(self, logical, source, base, file_id, display_name);
                e.insert(ms);
                Some(ms)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LogicalPaths(Vec<Vec<String>>);

unsafe impl salsa::Update for LogicalPaths {
    unsafe fn maybe_update(old_pointer: *mut Self, new_value: Self) -> bool {
        if unsafe { &*old_pointer } != &new_value {
            unsafe { *old_pointer = new_value };
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedModuleAst(Result<Module, String>);

impl PartialEq for ParsedModuleAst {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Ok(_), Ok(_)) | (Err(_), Err(_)) => true,
            _ => false,
        }
    }
}

unsafe impl salsa::Update for ParsedModuleAst {
    unsafe fn maybe_update(old_pointer: *mut Self, new_value: Self) -> bool {
        if unsafe { &*old_pointer } == &new_value {
            return false;
        }
        unsafe { *old_pointer = new_value };
        true
    }
}

#[salsa::tracked]
fn parse_module_source(db: &dyn DiscoveryDatabase, src: ModuleSource) -> ParsedModuleAst {
    let contents = src.contents(db);
    let file_id = src.file_id(db);
    let stream = easy::Stream(position::Stream::with_positioner(
        contents.as_str(),
        position::IndexPositioner::new(),
    ));
    match parser::parse_program(file_id).parse(stream) {
        Ok((module, _)) => ParsedModuleAst(Ok(module)),
        Err(e) => ParsedModuleAst(Err(format!(
            "Parse error in {}: {}",
            src.display_name(db),
            e
        ))),
    }
}

#[salsa::tracked]
fn module_direct_deps(db: &dyn DiscoveryDatabase, src: ModuleSource) -> LogicalPaths {
    let base = src.base(db);
    let ParsedModuleAst(result) = parse_module_source(db, src);
    let Ok(module) = result else {
        return LogicalPaths(vec![]);
    };
    LogicalPaths(deps_from_definitions(&base, &module.definitions))
}

#[salsa::tracked]
fn all_module_sources(db: &dyn DiscoveryDatabase, src: ModuleSource) -> LogicalPaths {
    let LogicalPaths(direct) = module_direct_deps(db, src);
    let mut all: Vec<Vec<String>> = vec![src.logical(db)];
    let mut seen: HashSet<Vec<String>> = HashSet::new();
    seen.insert(src.logical(db));

    for dep_logical in direct {
        if let Some(dep_src) = db.input(dep_logical) {
            let LogicalPaths(transitive) = all_module_sources(db, dep_src);
            for path in transitive {
                if seen.insert(path.clone()) {
                    all.push(path);
                }
            }
        }
    }
    LogicalPaths(all)
}

fn drain_inline_modules(
    module: &mut Module,
    parent_path: &NonEmpty<String>,
    file_id: FileId,
) -> Vec<DiscoveredModule> {
    let mut result = Vec::new();
    let mut remaining = Vec::new();

    for def in module.definitions.drain(..) {
        if let Definition::InlineModule {
            name,
            definitions,
            span,
        } = def
        {
            let mut child_logical: Vec<String> = parent_path.iter().cloned().collect();
            child_logical.push(name);
            let child_path = NonEmpty::from_vec(child_logical).unwrap();
            let mut child_module = Module {
                definitions,
                span,
                file_id,
            };
            let nested = drain_inline_modules(&mut child_module, &child_path, file_id);
            result.push(DiscoveredModule {
                name: child_path,
                module: child_module,
                is_entry: false,
                file_id,
                is_inline: true,
                source_info: None,
            });
            result.extend(nested);
        } else {
            remaining.push(def);
        }
    }

    module.definitions = remaining;
    result
}

fn deps_from_definitions(base: &[String], definitions: &[Definition]) -> Vec<Vec<String>> {
    let mut deps: Vec<Vec<String>> = vec![];

    for def in definitions {
        match def {
            Definition::Use { path, .. } => {
                let module_seg_count = path.len() - usize::from(path.len() > 1);
                let mut dep = base.to_vec();
                for seg in path.iter().take(module_seg_count) {
                    dep.push(seg.name.clone());
                    for param in &seg.params {
                        match param {
                            UseParam::Positional(names) => {
                                for pname in names {
                                    let mut pdep = base.to_vec();
                                    pdep.push(pname.clone());
                                    deps.push(pdep);
                                }
                            }
                            UseParam::Named { path: param_path, .. } => {
                                let mut pdep = base.to_vec();
                                pdep.extend(param_path.iter().cloned());
                                deps.push(pdep);
                            }
                        }
                    }
                }
                deps.push(dep);
            }
            Definition::ModuleHeader { params, .. } => {
                for param in params {
                    deps.push(vec![param.path.first().clone()]);
                }
            }
            Definition::InlineModule {
                definitions: inner_defs,
                ..
            } => {
                let inner_deps = deps_from_definitions(base, inner_defs);
                deps.extend(inner_deps);
            }
            _ => {}
        }
    }

    deps
}

pub(crate) fn discover_all(
    entry_path: &str,
    entry_source: &str,
    discoverer: Arc<dyn Discoverer>,
    native_module_names: &HashSet<String>,
    source_files: SourceFiles,
) -> Result<Vec<DiscoveredModule>, String> {
    let entry_dir = Path::new(entry_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".".to_string());

    let entry_stem = Path::new(entry_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("main")
        .to_string();

    let entry_logical = vec![entry_stem];

    let entry_file_id = source_files.add(entry_path.to_string(), entry_source.to_string());

    let db = DiscoveryDb::new(
        entry_dir,
        native_module_names.clone(),
        discoverer,
        source_files,
    );

    let entry_ms = ModuleSource::new(
        &db,
        entry_logical.clone(),
        entry_source.to_string(),
        vec![],
        entry_file_id,
        entry_path.to_string(),
    );
    db.sources.insert(entry_logical.clone(), entry_ms);

    let LogicalPaths(all_logical) = all_module_sources(&db, entry_ms);

    let mut all_modules: Vec<DiscoveredModule> = vec![];

    for logical in all_logical {
        let Some(ms) = db.input(logical.clone()) else {
            continue;
        };
        let ParsedModuleAst(parse_result) = parse_module_source(&db, ms);
        let mut module = match parse_result {
            Ok(m) => m,
            Err(e) => return Err(e),
        };
        let Some(ne_logical) = NonEmpty::from_vec(logical.clone()) else {
            continue;
        };
        let is_entry = logical == entry_logical;
        let file_id = ms.file_id(&db);
        let display_name = ms.display_name(&db);
        let contents = ms.contents(&db);
        let inline_mods = drain_inline_modules(&mut module, &ne_logical, file_id);

        all_modules.push(DiscoveredModule {
            name: ne_logical,
            module,
            is_entry,
            file_id,
            is_inline: false,
            source_info: Some((display_name, contents)),
        });

        all_modules.extend(inline_mods);
    }

    all_modules.sort_by_key(|m| m.file_id);

    Ok(all_modules)
}
