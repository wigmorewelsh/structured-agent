use super::refs::{
    CheckerAstRef, CheckerRefs, FunctionKind, NoWitness, SourceLocation, TypedCheckerAstRef,
    TypedRefs,
};
use crate::ast::{Definition, Module as AstModule, Type as AstType, TypeParam};
use crate::typed_ast;
use crate::types::{FileId, Span};
use nonempty::NonEmpty;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    FieldDefinition, FunctionDefinition, FunctionName, FunctionNameKind,
    GenericParameterDefinition, ImplDefinition, ImplKey, MetaData, ModuleDefinition, ModuleName,
    ParameterDefinition, SignatureEntry, TypeDefinition, TypeDefinitionKind, TypeName,
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
pub(super) fn get_function_sig<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: InternedFunctionName<'db>,
) -> Option<ArcPtr<super::FunctionSignature>> {
    let fn_name = name.name(db);
    let fn_def = lookup_function_def(db, tables, name)?;
    let kind = match &fn_def.get().ast_ref {
        CheckerAstRef::ExternalFn { .. } => FunctionKind::External,
        _ => FunctionKind::Bytecode,
    };
    let concrete_type = match &fn_name.kind {
        FunctionNameKind::Impl { type_name, .. } => Some(type_name.clone()),
        _ => None,
    };
    let type_key = InternedTypeName::new(db, fn_def.get().type_name.clone());
    let type_def = lookup_type_def(db, tables, type_key)?;
    let TypeDefinitionKind::Function {
        parameters,
        generic_parameters,
        return_type,
    } = &type_def.get().kind
    else {
        return None;
    };
    let type_params_vec: Vec<TypeParam> = generic_parameters
        .iter()
        .map(|gp| TypeParam {
            name: gp.name.clone(),
            bounds: gp.constraints.clone(),
        })
        .collect();
    let mut resolved_params = Vec::with_capacity(parameters.len());
    for p in parameters {
        let substituted = match &concrete_type {
            Some(ct) => super::TypeChecker::substitute_self(&p.type_name, ct),
            None => p.type_name.clone(),
        };
        let param_type = super::constraints::resolve(
            db,
            tables,
            &substituted,
            &fn_name.module,
            &type_params_vec,
            Span::dummy(),
            0,
        )?;
        resolved_params.push(crate::typed_ast::Parameter {
            name: p.name.clone(),
            param_type,
            span: Span::dummy(),
        });
    }
    let subst_return = match &concrete_type {
        Some(ct) => super::TypeChecker::substitute_self(return_type, ct),
        None => return_type.clone(),
    };
    let resolved_return = super::constraints::resolve(
        db,
        tables,
        &subst_return,
        &fn_name.module,
        &type_params_vec,
        Span::dummy(),
        0,
    )?;
    Some(ArcPtr::new(super::FunctionSignature {
        parameters: resolved_params,
        return_type: resolved_return,
        type_params: type_params_vec,
        kind,
    }))
}

pub(super) fn get_struct_fields(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: &str,
    current_module: &ModuleName,
) -> Option<(Vec<(String, AstType)>, Vec<TypeParam>)> {
    let interned_mod = current_module.intern(db);
    let interned_name = name.intern(db);
    let resolved =
        resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
            name: name.to_string(),
            module: current_module.clone(),
        });
    let key = InternedTypeName::new(db, resolved);
    lookup_type_def(db, tables, key).and_then(|arc_ptr| {
        if let CheckerAstRef::Struct(s) = &arc_ptr.get().ast_ref {
            let fields = s
                .fields
                .iter()
                .map(|f| (f.name.clone(), f.field_type.clone()))
                .collect();
            let type_params = s.type_params.clone();
            Some((fields, type_params))
        } else {
            None
        }
    })
}

#[salsa::tracked]
pub(super) fn check_module(
    db: &dyn TypeCheckDatabase,
    parsed: ParsedModuleInput,
    tables: SymbolTablesInput,
) {
    let module_name = ModuleName::new(parsed.name(db));
    let module = parsed.module(db);
    let ctx = super::CheckContext {
        file_id: parsed.file_id(db),
        module_name: &module_name,
    };
    for def in module.definitions.iter().filter(|def| {
        !matches!(
            def,
            Definition::ModuleHeader { .. } | Definition::Signature(_)
        )
    }) {
        super::elaboration::check_definition(db, tables, def, &ctx);
    }
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

pub(super) fn ast_type_to_type_name(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    ty: &AstType,
    module_name: &ModuleName,
) -> TypeName {
    let interned_mod = module_name.intern(db);
    let interned_name = ty.name.clone().intern(db);
    resolve_type_alias(db, tables, interned_mod, interned_name).unwrap_or_else(|| TypeName {
        name: ty.name.clone(),
        module: module_name.clone(),
    })
}

fn convert_generic_params(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    generic_parameters: &[GenericParameterDefinition<super::refs::CheckerRefs>],
    module: &ModuleName,
) -> Vec<GenericParameterDefinition<TypedRefs>> {
    generic_parameters
        .iter()
        .map(|gp| GenericParameterDefinition {
            name: gp.name.clone(),
            constraints: gp
                .constraints
                .iter()
                .map(|c| ast_type_to_type_name(db, tables, c, module))
                .collect(),
        })
        .collect()
}

fn convert_type_kind(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    kind: &TypeDefinitionKind<super::refs::CheckerRefs>,
    module: &ModuleName,
) -> TypeDefinitionKind<TypedRefs> {
    match kind {
        TypeDefinitionKind::Struct {
            fields,
            generic_parameters,
        } => TypeDefinitionKind::Struct {
            fields: fields
                .iter()
                .map(|f| FieldDefinition {
                    name: f.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &f.type_name, module),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
        },
        TypeDefinitionKind::Function {
            parameters,
            generic_parameters,
            return_type,
        } => TypeDefinitionKind::Function {
            parameters: parameters
                .iter()
                .map(|p| ParameterDefinition {
                    name: p.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &p.type_name, module),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
            return_type: ast_type_to_type_name(db, tables, return_type, module),
        },
        TypeDefinitionKind::Signature { entries } => TypeDefinitionKind::Signature {
            entries: entries
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &e.type_name, module),
                })
                .collect(),
        },
        TypeDefinitionKind::Trait { functions, .. } => TypeDefinitionKind::Trait {
            functions: functions
                .iter()
                .map(|e| SignatureEntry {
                    name: e.name.clone(),
                    type_name: ast_type_to_type_name(db, tables, &e.type_name, module),
                })
                .collect(),
            witness_ref: NoWitness,
        },
        TypeDefinitionKind::Primitive => TypeDefinitionKind::Primitive,
        TypeDefinitionKind::Native {
            generic_parameters,
            factory,
        } => TypeDefinitionKind::Native {
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
            factory: factory.clone(),
        },
    }
}

#[salsa::tracked]
pub(super) fn elaborate_function_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: InternedFunctionName<'db>,
) -> Option<ArcPtr<typed_ast::Function>> {
    let fn_def_ptr = lookup_function_def(db, tables, name)?;
    let fn_def = fn_def_ptr.get();
    match &fn_def.ast_ref {
        CheckerAstRef::Function(arc_fn, _) => {
            let ctx = super::CheckContext {
                file_id: fn_def.source_ref.0,
                module_name: &fn_def.name.module,
            };
            Some(ArcPtr::new(super::elaboration::elaborate_function(
                db, tables, arc_fn, &ctx,
            )?))
        }
        CheckerAstRef::ImplFunction(arc_fn, type_name_str, _) => {
            let concrete = super::TypeChecker::substitute_self_in_fn(arc_fn, type_name_str);
            let ctx = super::CheckContext {
                file_id: fn_def.source_ref.0,
                module_name: &fn_def.name.module,
            };
            Some(ArcPtr::new(super::elaboration::elaborate_function(
                db, tables, &concrete, &ctx,
            )?))
        }
        _ => None,
    }
}

#[salsa::tracked]
pub(super) fn elaborate_metadata(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
) -> ArcPtr<MetaData<TypedRefs>> {
    let mut typed_metadata: MetaData<TypedRefs> = MetaData::default();

    for fn_def in tables.functions(db).get().values() {
        let interned_name = (&fn_def.name).intern(db);
        let typed_ast_ref = match &fn_def.ast_ref {
            CheckerAstRef::Function(_, kind) => {
                match elaborate_function_def(db, tables, interned_name) {
                    Some(ptr) => TypedCheckerAstRef::Function(Arc::clone(&ptr.0), kind.clone()),
                    None => TypedCheckerAstRef::Other(fn_def.ast_ref.clone()),
                }
            }
            CheckerAstRef::ImplFunction(_, type_name_str, kind) => {
                match elaborate_function_def(db, tables, interned_name) {
                    Some(ptr) => TypedCheckerAstRef::ImplFunction(
                        Arc::clone(&ptr.0),
                        type_name_str.clone(),
                        kind.clone(),
                    ),
                    None => TypedCheckerAstRef::Other(fn_def.ast_ref.clone()),
                }
            }
            other => TypedCheckerAstRef::Other(other.clone()),
        };
        let typed_fn_def = FunctionDefinition {
            name: fn_def.name.clone(),
            visibility: fn_def.visibility.clone(),
            type_name: fn_def.type_name.clone(),
            source_ref: SourceLocation(fn_def.source_ref.0, fn_def.source_ref.1),
            ast_ref: typed_ast_ref,
            body_ref: None,
        };
        typed_metadata
            .functions
            .insert(fn_def.name.clone(), Arc::new(typed_fn_def));
    }

    for type_def in tables.types(db).get().values() {
        let kind = convert_type_kind(db, tables, &type_def.kind, &type_def.name.module);
        let new_def = TypeDefinition {
            name: type_def.name.clone(),
            kind,
            source_ref: SourceLocation(type_def.source_ref.0, type_def.source_ref.1),
            ast_ref: TypedCheckerAstRef::Other(type_def.ast_ref.clone()),
        };
        typed_metadata
            .types
            .insert(type_def.name.clone(), Arc::new(new_def));
    }

    for (impl_key, impl_def) in tables.impls(db).get() {
        let new_def = ImplDefinition {
            key: impl_def.key.clone(),
            module: impl_def.module.clone(),
            source_ref: SourceLocation(impl_def.source_ref.0, impl_def.source_ref.1),
            ast_ref: TypedCheckerAstRef::Other(impl_def.ast_ref.clone()),
        };
        typed_metadata
            .impls
            .insert(impl_key.clone(), Arc::new(new_def));
    }

    for (module_key, module_def) in tables.modules(db).get() {
        let new_def = ModuleDefinition {
            name: module_def.name.clone(),
            visibility: module_def.visibility.clone(),
            is_entry: module_def.is_entry,
            exports: module_def.exports.clone(),
            source_ref: SourceLocation(module_def.source_ref.0, module_def.source_ref.1),
            ast_ref: TypedCheckerAstRef::Other(module_def.ast_ref.clone()),
            use_imports: module_def.use_imports.clone(),
        };
        typed_metadata
            .modules
            .insert(module_key.clone(), Arc::new(new_def));
    }

    ArcPtr::new(typed_metadata)
}
