use super::refs::{CheckerAstRef, CheckerRefs, PrimitiveRefs};
use crate::ast::Module as AstModule;
use crate::types::FileId;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    FunctionDefinition, FunctionName, ImplDefinition, ImplKey, ModuleDefinition, ModuleName,
    TraitDefinition, TraitName, TypeDefinition, TypeName,
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
    pub(super) name: String,
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
    pub(super) traits: ArcPtr<HashMap<TraitName, Arc<TraitDefinition<CheckerRefs>>>>,
    pub(super) impls: ArcPtr<HashMap<ImplKey, Arc<ImplDefinition<CheckerRefs>>>>,
    pub(super) modules: ArcPtr<HashMap<ModuleName, Arc<ModuleDefinition<CheckerRefs>>>>,
    pub(super) param_bindings: ArcPtr<HashMap<ImplKey, Arc<ImplDefinition<PrimitiveRefs>>>>,
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
    pub(super) name: TraitName,
}

#[salsa::interned]
pub(super) struct InternedImplKey {
    pub(super) key: ImplKey,
}

#[salsa::interned]
pub(super) struct InternedModuleName {
    pub(super) name: ModuleName,
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
) -> Option<ArcPtr<TraitDefinition<CheckerRefs>>> {
    let name = key.name(db);
    tables
        .traits(db)
        .get()
        .get(&name)
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
        .any(|k| k.type_name.name == tn.name && k.trait_name.name == trn.name)
        || tables
            .param_bindings(db)
            .get()
            .keys()
            .any(|k| k.type_name.name == tn.name && k.trait_name.name == trn.name)
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
        .or_else(|| {
            tables
                .param_bindings(db)
                .get()
                .get(&impl_key)
                .map(|d| d.module.clone())
        })
}

#[salsa::tracked]
pub(super) fn find_trait_for_impl_call<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    fn_name: InternedString<'db>,
    type_name: InternedTypeName<'db>,
) -> Option<InternedTraitName<'db>> {
    for (trait_key, trait_def) in tables.traits(db).get() {
        let CheckerAstRef::Trait(ast_trait) = &trait_def.ast_ref else {
            continue;
        };
        if ast_trait
            .functions
            .iter()
            .any(|f| f.name == fn_name.value(db))
        {
            let interned_type = InternedTypeName::new(db, type_name.name(db).clone());
            let interned_trait = InternedTraitName::new(db, trait_key.clone());
            if lookup_impl_exists(db, tables, interned_type, interned_trait) {
                return Some(InternedTraitName::new(db, trait_key.clone()));
            }
        }
    }
    None
}
