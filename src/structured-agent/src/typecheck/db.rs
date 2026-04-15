use super::refs::{CheckerAstRef, CheckerRefs};
use crate::ast::{Definition, Module as AstModule};
use crate::typed_ast;
use crate::types::FileId;
use nonempty::NonEmpty;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    FunctionDefinition, FunctionName, FunctionNameKind, ImplDefinition, ImplKey, ModuleDefinition,
    ModuleName, TypeDefinition, TypeDefinitionKind, TypeName,
};

#[salsa::db]
pub(super) trait TypeCheckDatabase: salsa::Database {}

#[salsa::db]
#[derive(Default)]
pub(super) struct TypeCheckDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for TypeCheckDb {}

#[salsa::db]
impl TypeCheckDatabase for TypeCheckDb {}

#[salsa::input]
pub(super) struct ParsedModuleInput {
    pub(super) name: NonEmpty<String>,
    pub(super) is_entry: bool,
    pub(super) file_id: FileId,
    pub(super) module: AstModule,
}

pub(super) struct ArcPtr<T>(Arc<T>);

impl<T> ArcPtr<T> {
    pub(super) fn new(val: T) -> Self {
        ArcPtr(Arc::new(val))
    }

    pub(super) fn from_arc(arc: Arc<T>) -> Self {
        ArcPtr(arc)
    }

    pub(super) fn get(&self) -> &T {
        &self.0
    }
}

impl<T> Clone for ArcPtr<T> {
    fn clone(&self) -> Self {
        ArcPtr(Arc::clone(&self.0))
    }
}

impl<T> PartialEq for ArcPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Eq for ArcPtr<T> {}

impl<T: fmt::Debug> fmt::Debug for ArcPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl<T> Hash for ArcPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

unsafe impl<T> salsa::Update for ArcPtr<T> {
    unsafe fn maybe_update(old_pointer: *mut Self, new_value: Self) -> bool {
        #[allow(unsafe_op_in_unsafe_fn)]
        let old = &mut *old_pointer;
        if *old != new_value {
            *old = new_value;
            true
        } else {
            false
        }
    }
}

#[salsa::input]
pub(super) struct SymbolTablesInput {
    pub(super) functions: ArcPtr<HashMap<FunctionName, Arc<FunctionDefinition<CheckerRefs>>>>,
    pub(super) types: ArcPtr<HashMap<TypeName, Arc<TypeDefinition<CheckerRefs>>>>,
    pub(super) impls: ArcPtr<HashMap<ImplKey, Arc<ImplDefinition<CheckerRefs>>>>,
    pub(super) modules: ArcPtr<HashMap<ModuleName, Arc<ModuleDefinition<CheckerRefs>>>>,
}

#[salsa::interned]
pub(super) struct InternedString<'db> {
    pub(super) value: String,
}

#[salsa::interned]
pub(super) struct InternedFunctionName {
    pub(super) name: FunctionName,
}

#[salsa::interned]
pub(super) struct InternedTypeName {
    pub(super) name: TypeName,
}

#[salsa::interned]
pub(super) struct InternedTraitName {
    pub(super) name: TypeName,
}

#[salsa::interned]
pub(super) struct InternedImplKey {
    pub(super) key: ImplKey,
}

#[salsa::interned]
pub(super) struct InternedModuleName {
    pub(super) name: ModuleName,
}

pub(super) trait Intern<'db> {
    type Interned;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned;
}

impl<'db> Intern<'db> for ModuleName {
    type Interned = InternedModuleName<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedModuleName::new(db, self)
    }
}

impl<'db> Intern<'db> for &ModuleName {
    type Interned = InternedModuleName<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedModuleName::new(db, self.clone())
    }
}

impl<'db> Intern<'db> for String {
    type Interned = InternedString<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedString::new(db, self)
    }
}

impl<'db> Intern<'db> for &String {
    type Interned = InternedString<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedString::new(db, self.clone())
    }
}

impl<'db> Intern<'db> for &str {
    type Interned = InternedString<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedString::new(db, self.to_string())
    }
}

impl<'db> Intern<'db> for FunctionName {
    type Interned = InternedFunctionName<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedFunctionName::new(db, self)
    }
}

impl<'db> Intern<'db> for &FunctionName {
    type Interned = InternedFunctionName<'db>;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned {
        InternedFunctionName::new(db, self.clone())
    }
}

#[salsa::tracked]
pub(super) fn lookup_function_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    key: InternedFunctionName<'db>,
) -> Option<ArcPtr<FunctionDefinition<CheckerRefs>>> {
    let name = key.name(db);
    tables
        .functions(db)
        .get()
        .get(&name)
        .map(|arc| ArcPtr::from_arc(arc.clone()))
}

#[salsa::tracked]
pub(super) fn lookup_type_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    key: InternedTypeName<'db>,
) -> Option<ArcPtr<TypeDefinition<CheckerRefs>>> {
    let name = key.name(db);
    tables
        .types(db)
        .get()
        .get(&name)
        .map(|arc| ArcPtr::from_arc(arc.clone()))
}

#[salsa::tracked]
pub(super) fn lookup_trait_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    key: InternedTraitName<'db>,
) -> Option<ArcPtr<TypeDefinition<CheckerRefs>>> {
    let name = key.name(db);
    let type_name = TypeName {
        name: name.name,
        module: name.module,
    };
    tables
        .types(db)
        .get()
        .get(&type_name)
        .filter(|td| matches!(td.kind, TypeDefinitionKind::Trait { .. }))
        .map(|arc| ArcPtr::from_arc(arc.clone()))
}

#[salsa::tracked]
pub(super) fn lookup_impl_exists<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    type_name: InternedTypeName<'db>,
    trait_name: InternedTraitName<'db>,
) -> bool {
    let tn = type_name.name(db);
    let trn = trait_name.name(db);
    tables
        .impls(db)
        .get()
        .keys()
        .any(|k| k.type_name == tn.name && k.trait_name == trn.name)
}

#[salsa::tracked]
pub(super) fn lookup_impl_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    key: InternedImplKey<'db>,
) -> Option<ModuleName> {
    let impl_key = key.key(db);
    tables
        .impls(db)
        .get()
        .get(&impl_key)
        .map(|d| d.module.clone())
}

#[salsa::tracked]
pub(super) fn check_module(
    db: &dyn TypeCheckDatabase,
    parsed: ParsedModuleInput,
    tables: SymbolTablesInput,
) -> Option<ArcPtr<typed_ast::Module>> {
    let module_name = ModuleName::new(parsed.name(db));
    let module = parsed.module(db);
    let ctx = super::CheckContext {
        file_id: parsed.file_id(db),
        module_name: &module_name,
    };
    let typed_definitions = module
        .definitions
        .iter()
        .filter(|def| {
            !matches!(
                def,
                Definition::ModuleHeader { .. } | Definition::Signature(_)
            )
        })
        .filter_map(|def| super::elaboration::check_definition(db, tables, def, &ctx))
        .collect::<Vec<_>>();
    Some(ArcPtr::new(typed_ast::Module {
        definitions: typed_definitions,
        span: module.span,
        file_id: parsed.file_id(db),
    }))
}

#[salsa::tracked]
pub(super) fn find_trait_for_impl_call<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    fn_name: InternedString<'db>,
    type_name: InternedTypeName<'db>,
) -> Option<InternedTraitName<'db>> {
    for (type_key, type_def) in tables.types(db).get() {
        let TypeDefinitionKind::Trait { .. } = &type_def.kind else {
            continue;
        };
        let CheckerAstRef::Trait(ast_trait) = &type_def.ast_ref else {
            continue;
        };
        if ast_trait
            .functions
            .iter()
            .any(|f| f.name == fn_name.value(db))
        {
            let trait_name = TypeName {
                name: type_key.name.clone(),
                module: type_key.module.clone(),
            };
            let interned_type = InternedTypeName::new(db, type_name.name(db).clone());
            let interned_trait = InternedTraitName::new(db, trait_name.clone());
            if lookup_impl_exists(db, tables, interned_type, interned_trait) {
                return Some(InternedTraitName::new(db, trait_name));
            }
        }
    }
    None
}

#[salsa::tracked]
pub(super) fn resolve_type_in_module<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<TypeName> {
    let key = TypeName {
        name: symbol.value(db),
        module: module.name(db),
    };
    tables.types(db).get().get(&key).map(|_| key)
}

fn export_type_cycle_recovery<'db>(
    _db: &'db dyn TypeCheckDatabase,
    _id: salsa::Id,
    _tables: SymbolTablesInput,
    _module: InternedModuleName<'db>,
    _symbol: InternedString<'db>,
) -> Option<TypeName> {
    None
}

#[salsa::tracked(cycle_result = export_type_cycle_recovery)]
pub(super) fn module_exports_type<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<TypeName> {
    if let Some(found) = resolve_type_in_module(db, tables, module, symbol) {
        return Some(found);
    }
    let module_name = module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
    let symbol_str = symbol.value(db);
    for import in &module_def.use_imports {
        if import.is_pub && import.local == symbol_str {
            let import_module = (&import.module).intern(db);
            let import_name = (&import.name).intern(db);
            if let Some(found) = module_exports_type(db, tables, import_module, import_name) {
                return Some(found);
            }
        }
    }
    None
}

#[salsa::tracked]
pub(super) fn resolve_function_in_module<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<FunctionName> {
    let key = FunctionName {
        name: symbol.value(db),
        module: module.name(db),
        kind: FunctionNameKind::Function,
    };
    tables.functions(db).get().get(&key).map(|_| key)
}

fn export_function_cycle_recovery<'db>(
    _db: &'db dyn TypeCheckDatabase,
    _id: salsa::Id,
    _tables: SymbolTablesInput,
    _module: InternedModuleName<'db>,
    _symbol: InternedString<'db>,
) -> Option<FunctionName> {
    None
}

#[salsa::tracked(cycle_result = export_function_cycle_recovery)]
pub(super) fn module_exports_function<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<FunctionName> {
    if let Some(found) = resolve_function_in_module(db, tables, module, symbol) {
        return Some(found);
    }
    let module_name = module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
    let symbol_str = symbol.value(db);
    for import in &module_def.use_imports {
        if import.is_pub && import.local == symbol_str {
            let import_module = (&import.module).intern(db);
            let import_name = (&import.name).intern(db);
            if let Some(found) = module_exports_function(db, tables, import_module, import_name) {
                return Some(found);
            }
        }
    }
    None
}

#[salsa::tracked]
pub(super) fn resolve_module_path<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    use_path: InternedModuleName<'db>,
) -> ModuleName {
    let current_name = current_module.name(db);
    let use_path_name = use_path.name(db);
    let module_alias = use_path_name.segments.first();
    let module_def = tables.modules(db).get().get(&current_name).cloned();
    let header_params = module_def
        .as_ref()
        .and_then(|m| {
            let CheckerAstRef::Module(ast) = &m.ast_ref else {
                return None;
            };
            ast.definitions.iter().find_map(|d| {
                let Definition::ModuleHeader { params, .. } = d else {
                    return None;
                };
                Some(params.clone())
            })
        })
        .unwrap_or_default();

    if let Some(param) = header_params.iter().find(|p| &p.name == module_alias) {
        let mut resolved: Vec<String> = param.path.iter().cloned().collect();
        resolved.extend(use_path_name.segments.iter().skip(1).cloned());
        return ModuleName::new(NonEmpty::from_vec(resolved).unwrap());
    }

    let mut resolved: Vec<String> = current_name.segments.iter().cloned().collect();
    resolved.pop();
    resolved.extend(use_path_name.segments.iter().cloned());
    ModuleName::new(NonEmpty::from_vec(resolved).unwrap_or(use_path_name.segments.clone()))
}

#[salsa::tracked]
pub(super) fn resolve_type_alias<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    alias: InternedString<'db>,
) -> Option<TypeName> {
    let module_name = current_module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
    let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
        return None;
    };
    let alias_str = alias.value(db);
    for def in &ast_module.definitions {
        if let Definition::Use {
            path,
            name,
            alias: use_alias,
            ..
        } = def
        {
            let effective = use_alias
                .as_ref()
                .map(String::as_str)
                .unwrap_or(name.as_str());
            if effective == alias_str.as_str() {
                let use_path = ModuleName::new(path.clone()).intern(db);
                let resolved = resolve_module_path(db, tables, current_module, use_path);
                let resolved_module = resolved.intern(db);
                let resolved_name = name.intern(db);
                return module_exports_type(db, tables, resolved_module, resolved_name);
            }
        }
    }
    for import in &module_def.use_imports {
        if import.local == alias_str {
            let import_module = (&import.module).intern(db);
            let import_name = (&import.name).intern(db);
            if let Some(found) = module_exports_type(db, tables, import_module, import_name) {
                return Some(found);
            }
        }
    }
    None
}

#[salsa::tracked]
pub(super) fn resolve_function_alias<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    alias: InternedString<'db>,
) -> Option<FunctionName> {
    let module_name = current_module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
    let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
        return None;
    };
    let alias_str = alias.value(db);
    for def in &ast_module.definitions {
        if let Definition::Use {
            path,
            name,
            alias: use_alias,
            ..
        } = def
        {
            let effective = use_alias
                .as_ref()
                .map(String::as_str)
                .unwrap_or(name.as_str());
            if effective == alias_str.as_str() {
                let use_path = ModuleName::new(path.clone()).intern(db);
                let resolved = resolve_module_path(db, tables, current_module, use_path);
                let resolved_module = resolved.intern(db);
                let resolved_name = name.intern(db);
                return module_exports_function(db, tables, resolved_module, resolved_name);
            }
        }
    }
    None
}

#[salsa::tracked]
pub(super) fn resolve_function_call<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<FunctionName> {
    resolve_function_alias(db, tables, current_module, symbol)
        .or_else(|| resolve_function_in_module(db, tables, current_module, symbol))
}
