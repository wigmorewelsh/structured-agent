use super::refs::{
    CheckerAstRef, CheckerRefs, FunctionKind, NoWitness, SourceLocation, TypedCheckerAstRef,
    TypedRefs,
};
use crate::TypeError;
use crate::error::OrAccumulateError;
use structured_agent_ast::ast::{
    Definition, Module as AstModule, PathArg, PathSegment, Type as AstType, TypeParam, Use,
};

use nonempty::NonEmpty;
use structured_agent_ast::types::{FileId, Span};
use structured_agent_typed_ast as typed_ast;

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    DefinitionPath, FieldDefinition, FunctionDefinition, GenericParameterDefinition,
    ImplDefinition, MetaData, ModuleDefinition, ParameterDefinition, TypeDefinition,
    TypeDefinitionKind,
};

#[salsa::db]
pub trait TypeCheckDatabase: salsa::Database {
    fn symbol_tables(&self) -> SymbolTablesInput;
}

#[salsa::db]
pub struct TypeCheckDb {
    storage: salsa::Storage<Self>,
    symbol_tables: Option<SymbolTablesInput>,
}

impl Default for TypeCheckDb {
    fn default() -> Self {
        Self {
            storage: Default::default(),
            symbol_tables: None,
        }
    }
}

#[salsa::db]
impl salsa::Database for TypeCheckDb {}

#[salsa::db]
impl TypeCheckDatabase for TypeCheckDb {
    fn symbol_tables(&self) -> SymbolTablesInput {
        self.symbol_tables.expect("symbol tables not set")
    }
}

impl TypeCheckDb {
    pub fn set_symbol_tables(&mut self, tables: SymbolTablesInput) {
        self.symbol_tables = Some(tables);
    }
}

#[salsa::input]
pub struct ParsedModuleInput {
    pub name: NonEmpty<String>,
    pub is_entry: bool,
    pub file_id: FileId,
    pub module: AstModule,
}

#[salsa::input]
pub struct ProgramInput {
    pub modules: Vec<ParsedModuleInput>,
}

pub struct ArcPtr<T>(Arc<T>);

impl<T> ArcPtr<T> {
    pub fn new(val: T) -> Self {
        ArcPtr(Arc::new(val))
    }

    pub fn from_arc(arc: Arc<T>) -> Self {
        ArcPtr(arc)
    }

    pub fn get(&self) -> &T {
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
pub struct SymbolTablesInput {
    pub functions: ArcPtr<HashMap<DefinitionPath, Arc<FunctionDefinition<CheckerRefs>>>>,
    pub types: ArcPtr<HashMap<DefinitionPath, Arc<TypeDefinition<CheckerRefs>>>>,
    pub impls: ArcPtr<HashMap<DefinitionPath, Arc<ImplDefinition<CheckerRefs>>>>,
    pub modules: ArcPtr<HashMap<DefinitionPath, Arc<ModuleDefinition<CheckerRefs>>>>,
}

#[salsa::interned]
pub struct InternedString<'db> {
    pub value: String,
}

#[salsa::interned]
pub struct InternedFunctionName {
    pub name: DefinitionPath,
}

#[salsa::interned]
pub struct InternedTypeName {
    pub name: DefinitionPath,
}

#[salsa::interned]
pub struct InternedTraitName {
    pub name: DefinitionPath,
}

#[salsa::interned]
pub struct InternedImplKey {
    pub key: DefinitionPath,
}

#[salsa::interned]
pub struct InternedModuleName {
    pub name: DefinitionPath,
}

pub trait Intern<'db> {
    type Interned;
    fn intern(self, db: &'db dyn TypeCheckDatabase) -> Self::Interned;
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

#[salsa::tracked]
pub fn lookup_function_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    key: InternedFunctionName<'db>,
) -> Option<ArcPtr<FunctionDefinition<CheckerRefs>>> {
    let name = key.name(db);
    db.symbol_tables()
        .functions(db)
        .get()
        .get(&name)
        .map(|arc| ArcPtr::from_arc(arc.clone()))
}

#[salsa::tracked]
pub fn lookup_type_def_in_symbol_tables<'db>(
    db: &'db dyn TypeCheckDatabase,
    key: InternedTypeName<'db>,
) -> Option<ArcPtr<TypeDefinition<CheckerRefs>>> {
    let name = key.name(db);
    db.symbol_tables()
        .types(db)
        .get()
        .get(&name)
        .map(|arc| ArcPtr::from_arc(arc.clone()))
}

#[salsa::tracked]
pub fn get_function_sig<'db>(
    db: &'db dyn TypeCheckDatabase,
    name: InternedFunctionName<'db>,
    program: ProgramInput,
) -> Option<ArcPtr<super::FunctionSignature>> {
    let fn_name = name.name(db);
    let fn_def = lookup_function_def(db, name).or_accumulate(
        db,
        TypeError::UndefinedType {
            name: fn_name.to_string(),
            span: Span::dummy(),
            file_id: 0,
        },
    )?;
    let kind = match &fn_def.get().ast_ref {
        CheckerAstRef::ExternalFn { .. } => FunctionKind::External,
        _ => FunctionKind::Bytecode,
    };

    let type_key = InternedTypeName::new(db, fn_def.get().type_name.clone());
    let type_def = lookup_type_def_in_symbol_tables(db, type_key).or_accumulate(
        db,
        TypeError::UndefinedType {
            name: fn_def.get().type_name.last_name().to_string(),
            span: fn_def.get().source_ref.1,
            file_id: fn_def.get().source_ref.0,
        },
    )?;
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
    let mut type_env = super::TypeEnvironment::with_type_params(&type_params_vec);
    if let Some(impl_key) = fn_name.impl_key() {
        let impls = db.symbol_tables().impls(db);
        if let Some(impl_def) = impls.get().get(&impl_key) {
            let self_type = DefinitionPath::for_type(
                impl_def.module.clone(),
                impl_def.type_name.name().to_string(),
            );
            type_env.set_self_type(self_type);
        }
    }

    let mut resolved_params = Vec::with_capacity(parameters.len());
    for p in parameters {
        let param_ctx = super::CheckContext {
            file_id: p.source_ref.0,
            module_name: &fn_name.module_prefix(),
            program,
        };
        let param_type =
            super::synthesize::resolve(db, &p.type_name, &type_env, p.source_ref.1, &param_ctx)?;
        resolved_params.push(crate::typed_ast::Parameter {
            name: p.name.clone(),
            param_type,
            binding_id: crate::typed_ast::BindingId(0),
            span: p.source_ref.1,
        });
    }
    let return_ctx = super::CheckContext {
        file_id: type_def.get().source_ref.0,
        module_name: &fn_name.module_prefix(),
        program,
    };
    let resolved_return = super::synthesize::resolve(
        db,
        return_type,
        &type_env,
        type_def.get().source_ref.1,
        &return_ctx,
    )?;
    Some(ArcPtr::new(super::FunctionSignature {
        parameters: resolved_params,
        return_type: resolved_return,
        type_params: type_params_vec,
        kind,
    }))
}

pub fn get_struct_fields(
    db: &dyn TypeCheckDatabase,
    name: &str,
    current_module: &DefinitionPath,
) -> Option<(Vec<(String, AstType)>, Vec<TypeParam>)> {
    let interned_mod = InternedModuleName::new(db, current_module.clone());
    let interned_name = name.intern(db);
    let resolved = resolve_type_in_module(db, interned_mod, interned_name)
        .map(|r| r.ty)
        .unwrap_or_else(|| DefinitionPath::for_type(current_module.clone(), name));
    let key = InternedTypeName::new(db, resolved);
    lookup_type_def_in_symbol_tables(db, key).and_then(|arc_ptr| {
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

pub fn find_impl_fn(
    db: &dyn TypeCheckDatabase,
    type_name: &str,
    method_name: &str,
    current_module: &DefinitionPath,
) -> Option<DefinitionPath> {
    let impls = db.symbol_tables().impls(db);
    let impl_entry = impls
        .get()
        .values()
        .find(|i| i.type_name.name() == type_name && i.module == *current_module)?;
    Some(DefinitionPath::for_impl_fn(&impl_entry.key, method_name))
}

pub fn impl_for_type_and_trait(
    solved: &crate::solver::SolvedConstraints,
    type_path: &DefinitionPath,
    trait_path: &DefinitionPath,
) -> Option<DefinitionPath> {
    solved
        .impls
        .get(&(type_path.clone(), trait_path.clone()))
        .cloned()
}

#[salsa::tracked]
pub fn check_program(db: &dyn TypeCheckDatabase, program: ProgramInput) {
    for parsed in program.modules(db) {
        check_module(db, parsed, program);
    }
}

#[salsa::tracked]
pub fn check_module(db: &dyn TypeCheckDatabase, parsed: ParsedModuleInput, program: ProgramInput) {
    let module_name = DefinitionPath::for_module(parsed.name(db));
    let module = parsed.module(db);
    let ctx = super::CheckContext {
        file_id: parsed.file_id(db),
        module_name: &module_name,
        program,
    };
    for def in module.definitions.iter().filter(|def| {
        !matches!(
            def,
            Definition::ModuleHeader { .. } | Definition::Signature(_)
        )
    }) {
        super::synthesize::check_definition(db, def, &ctx);
    }
}

#[salsa::tracked]
pub fn lookup_type_in_symbol_tables<'db>(
    db: &'db dyn TypeCheckDatabase,
    module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let key = DefinitionPath::for_type(module.name(db), symbol.value(db));
    db.symbol_tables().types(db).get().get(&key).map(|_| key)
}

mod type_resolver {
    use super::*;

    type DefKind = TypeDefinitionKind<CheckerRefs>;

    #[derive(Clone, PartialEq)]
    pub struct ResolvedType {
        pub ty: DefinitionPath,
        pub path: Vec<ResolveSegment>,
    }

    impl ResolvedType {
        pub fn new(ty: DefinitionPath) -> Self {
            Self { ty, path: vec![] }
        }

        pub fn with_segment(mut self, seg: ResolveSegment) -> Self {
            self.path.insert(0, seg);
            self
        }
    }

    unsafe impl salsa::Update for ResolvedType {
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

    #[derive(Clone, PartialEq)]
    #[allow(dead_code)]
    pub enum ResolveSegment {
        Local(DefinitionPath, Vec<PathArg>),
        UseAlias(String, DefinitionPath, Vec<PathArg>),
        UseDirect(String, DefinitionPath, Vec<PathArg>),
        ModuleHeader(String, DefinitionPath, Vec<PathArg>),
    }

    impl ResolveSegment {
        fn inject_params(&mut self, params: Vec<PathArg>) {
            match self {
                ResolveSegment::Local(_, p)
                | ResolveSegment::UseAlias(_, _, p)
                | ResolveSegment::UseDirect(_, _, p)
                | ResolveSegment::ModuleHeader(_, _, p) => *p = params,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct ModuleInstantiation {
        pub path: DefinitionPath,
        pub params: Vec<ModuleInstantiation>,
    }

    pub enum CallModuleArg {
        Concrete(ModuleInstantiation),
        FromParam(String),
    }

    pub struct CallRouting {
        pub via_module_param: Option<String>,
        pub module_args: Vec<CallModuleArg>,
    }

    pub fn resolve_absolute_path<'db>(
        db: &'db dyn TypeCheckDatabase,
        use_path: NonEmpty<PathSegment>,
    ) -> Option<DefinitionPath> {
        let mut search_module = InternedModuleName::new(db, DefinitionPath::root());
        let mut last_type_name = None;
        for symbol in use_path.iter() {
            let resolved =
                resolve_type_in_module(db, search_module, symbol.name.clone().intern(db))?;
            let type_def = lookup_type_def_in_symbol_tables(
                db,
                InternedTypeName::new(db, resolved.ty.clone()),
            )?;
            if let DefKind::Signature { .. } = type_def.get().kind {
                search_module = InternedModuleName::new(db, resolved.ty.clone());
            }
            last_type_name = Some(resolved.ty);
        }
        last_type_name
    }

    fn resolve_local_use_path<'db>(
        db: &'db dyn TypeCheckDatabase,
        current_module: InternedModuleName<'db>,
        use_path: Arc<Use>,
    ) -> Option<ResolvedType> {
        let mut search_module = current_module;
        let mut accumulated: Vec<ResolveSegment> = vec![];
        let mut last_ty = None;
        for seg in use_path.path.iter() {
            let symbol = seg.name.clone().intern(db);
            let mut resolved = resolve_type_in_module(db, search_module, symbol)?;
            if !seg.params.is_empty() {
                if let Some(last_seg) = resolved.path.last_mut() {
                    last_seg.inject_params(seg.params.clone());
                }
            }
            accumulated.extend(resolved.path);
            if let Some(type_def) =
                lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, resolved.ty.clone()))
            {
                if let DefKind::Signature { .. } = type_def.get().kind {
                    search_module = InternedModuleName::new(db, resolved.ty.clone());
                }
            }
            last_ty = Some(resolved.ty);
        }
        Some(ResolvedType {
            ty: last_ty?,
            path: accumulated,
        })
    }

    fn resolve_type_cycle_recovery<'db>(
        _db: &'db dyn TypeCheckDatabase,
        _id: salsa::Id,
        _current_module: InternedModuleName<'db>,
        _symbol: InternedString<'db>,
    ) -> Option<ResolvedType> {
        None
    }

    #[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
    pub fn resolve_type_in_module<'db>(
        db: &'db dyn TypeCheckDatabase,
        current_module: InternedModuleName<'db>,
        symbol: InternedString<'db>,
    ) -> Option<ResolvedType> {
        let prelude = InternedModuleName::new(
            db,
            DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
        );
        let unstable = InternedModuleName::new(
            db,
            DefinitionPath::for_module(NonEmpty::new("unstable".to_string())),
        );
        lookup_type_in_symbol_tables(db, current_module, symbol)
            .map(|ty| {
                ResolvedType::new(ty.clone())
                    .with_segment(ResolveSegment::Local(current_module.name(db), vec![]))
            })
            .or_else(|| resolve_type_as_mod_param(db, current_module, symbol))
            .or_else(|| resolve_type_as_alias(db, current_module, symbol))
            .or_else(|| resolve_type_as_use(db, current_module, symbol))
            .or_else(|| {
                lookup_type_in_symbol_tables(db, prelude, symbol).map(|ty| {
                    ResolvedType::new(ty.clone())
                        .with_segment(ResolveSegment::Local(prelude.name(db), vec![]))
                })
            })
            .or_else(|| {
                lookup_type_in_symbol_tables(db, unstable, symbol).map(|ty| {
                    ResolvedType::new(ty.clone())
                        .with_segment(ResolveSegment::Local(unstable.name(db), vec![]))
                })
            })
            .or_else(|| resolve_type_as_sibling_module(db, current_module, symbol))
    }

    #[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
    fn resolve_type_as_sibling_module<'db>(
        db: &'db dyn TypeCheckDatabase,
        current_module: InternedModuleName<'db>,
        symbol: InternedString<'db>,
    ) -> Option<ResolvedType> {
        let key = current_module
            .name(db)
            .parent()
            .with_module(symbol.value(db));
        db.symbol_tables().types(db).get().get(&key).map(|_| {
            ResolvedType::new(key.clone()).with_segment(ResolveSegment::Local(
                current_module.name(db).parent(),
                vec![],
            ))
        })
    }

    #[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
    fn resolve_type_as_mod_param<'db>(
        db: &'db dyn TypeCheckDatabase,
        current_module: InternedModuleName<'db>,
        symbol: InternedString<'db>,
    ) -> Option<ResolvedType> {
        let module_def = db
            .symbol_tables()
            .modules(db)
            .get()
            .get(&current_module.name(db))?
            .clone();
        let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
            return None;
        };
        for def in &ast_module.definitions {
            if let Definition::ModuleHeader { params, .. } = def {
                for param in params {
                    if param.name == symbol.value(db) {
                        let ty = resolve_absolute_path(db, param.path.clone())?;
                        return Some(ResolvedType::new(ty).with_segment(
                            ResolveSegment::ModuleHeader(
                                param.name.clone(),
                                current_module.name(db),
                                vec![],
                            ),
                        ));
                    }
                }
            }
        }
        None
    }

    #[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
    fn resolve_type_as_alias<'db>(
        db: &'db dyn TypeCheckDatabase,
        current_module: InternedModuleName<'db>,
        symbol: InternedString<'db>,
    ) -> Option<ResolvedType> {
        let module_name = current_module.name(db);
        let module_def = db
            .symbol_tables()
            .modules(db)
            .get()
            .get(&module_name)?
            .clone();
        let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
            return None;
        };
        let alias_str = symbol.value(db);
        for def in &ast_module.definitions {
            if let Definition::Use(u) = def
                && let Some(use_alias) = &u.alias
            {
                if use_alias == alias_str.as_str() {
                    return resolve_local_use_path(db, current_module, u.clone());
                }
            }
        }
        None
    }

    #[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
    fn resolve_type_as_use<'db>(
        db: &'db dyn TypeCheckDatabase,
        current_module: InternedModuleName<'db>,
        symbol: InternedString<'db>,
    ) -> Option<ResolvedType> {
        let module_name = current_module.name(db);
        let module_def = db
            .symbol_tables()
            .modules(db)
            .get()
            .get(&module_name)?
            .clone();
        let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
            return None;
        };
        let symbol_str = symbol.value(db);
        for def in &ast_module.definitions {
            if let Definition::Use(u) = def
                && u.alias.is_none()
            {
                let last = u.path.last();
                if last.name == symbol_str.as_str() {
                    return resolve_local_use_path(db, current_module, u.clone());
                }
            }
        }
        None
    }
}

pub use type_resolver::{
    CallModuleArg, CallRouting, ModuleInstantiation, ResolveSegment, ResolvedType,
    resolve_type_in_module,
};

type DefKind = TypeDefinitionKind<CheckerRefs>;

fn build_module_instantiation<'db>(
    db: &'db dyn TypeCheckDatabase,
    current_module: InternedModuleName<'db>,
    path: &NonEmpty<PathSegment>,
) -> Option<ModuleInstantiation> {
    let resolved = resolve_path_full(db, current_module, path)?;
    let params: Vec<ModuleInstantiation> = resolved
        .path
        .iter()
        .flat_map(|seg| match seg {
            ResolveSegment::Local(_, p)
            | ResolveSegment::UseAlias(_, _, p)
            | ResolveSegment::UseDirect(_, _, p)
            | ResolveSegment::ModuleHeader(_, _, p) => p.iter(),
        })
        .filter_map(|use_param| {
            let param_path = match use_param {
                PathArg::Positional(p) => p,
                PathArg::Named { path, .. } => path,
            };
            build_module_instantiation(db, current_module, param_path)
        })
        .collect();
    Some(ModuleInstantiation {
        path: resolved.ty,
        params,
    })
}

pub fn resolve_call_routing<'db>(
    db: &'db dyn TypeCheckDatabase,
    current_module: InternedModuleName<'db>,
    alias: InternedString<'db>,
) -> Option<CallRouting> {
    let resolved = resolve_type_in_module(db, current_module, alias)?;
    let via_module_param =
        if let Some(ResolveSegment::ModuleHeader(name, _, _)) = resolved.path.first() {
            Some(name.clone())
        } else {
            None
        };
    let module_args = resolved
        .path
        .iter()
        .flat_map(|seg| match seg {
            ResolveSegment::Local(_, p)
            | ResolveSegment::UseAlias(_, _, p)
            | ResolveSegment::UseDirect(_, _, p)
            | ResolveSegment::ModuleHeader(_, _, p) => p.iter(),
        })
        .filter_map(|use_param| {
            let path = match use_param {
                PathArg::Positional(path_segs) => path_segs,
                PathArg::Named { path, .. } => path,
            };
            let resolved = resolve_path_full(db, current_module, path)?;
            match resolved.path.iter().find_map(|s| {
                if let ResolveSegment::ModuleHeader(name, _, _) = s {
                    Some(name.clone())
                } else {
                    None
                }
            }) {
                Some(param_name) => Some(CallModuleArg::FromParam(param_name)),
                None => build_module_instantiation(db, current_module, path)
                    .map(CallModuleArg::Concrete),
            }
        })
        .collect();
    Some(CallRouting {
        via_module_param,
        module_args,
    })
}

#[salsa::tracked]
pub fn resolve_function_call<'db>(
    db: &'db dyn TypeCheckDatabase,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let type_name = resolve_type_in_module(db, current_module, symbol)?.ty;
    let type_def =
        lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, type_name.clone()))?;
    if let DefKind::Function { .. } = type_def.get().kind {
        Some(DefinitionPath::for_function(
            type_name.module_prefix(),
            type_name.last_name(),
        ))
    } else {
        None
    }
}

fn resolve_path_full<'db>(
    db: &'db dyn TypeCheckDatabase,
    current_module: InternedModuleName<'db>,
    path: &NonEmpty<PathSegment>,
) -> Option<ResolvedType> {
    let mut search = current_module;
    let mut accumulated: Vec<ResolveSegment> = vec![];
    let mut last_ty = None;
    for seg in path.iter() {
        let sym = seg.name.clone().intern(db);
        let resolved = resolve_type_in_module(db, search, sym)?;
        accumulated.extend(resolved.path);
        if let Some(type_def) =
            lookup_type_def_in_symbol_tables(db, InternedTypeName::new(db, resolved.ty.clone()))
        {
            if let DefKind::Signature { .. } = type_def.get().kind {
                search = InternedModuleName::new(db, resolved.ty.clone());
            }
        }
        last_ty = Some(resolved.ty);
    }
    Some(ResolvedType {
        ty: last_ty?,
        path: accumulated,
    })
}

fn resolve_path_to_module<'db>(
    db: &'db dyn TypeCheckDatabase,
    current_module: InternedModuleName<'db>,
    path: &NonEmpty<PathSegment>,
) -> Option<DefinitionPath> {
    resolve_path_full(db, current_module, path).map(|r| r.ty)
}

pub fn ast_type_to_type_name(
    db: &dyn TypeCheckDatabase,
    ty: &AstType,
    module_name: &DefinitionPath,
) -> DefinitionPath {
    let interned_mod = InternedModuleName::new(db, module_name.clone());
    let interned_name = ty.name().to_string().intern(db);
    resolve_type_in_module(db, interned_mod, interned_name)
        .map(|r| r.ty)
        .unwrap_or_else(|| DefinitionPath::for_type(module_name.clone(), ty.name().to_string()))
}

fn convert_generic_params(
    db: &dyn TypeCheckDatabase,
    generic_parameters: &[GenericParameterDefinition<super::refs::CheckerRefs>],
    module: &DefinitionPath,
) -> Vec<GenericParameterDefinition<TypedRefs>> {
    generic_parameters
        .iter()
        .map(|gp| GenericParameterDefinition {
            name: gp.name.clone(),
            constraints: gp
                .constraints
                .iter()
                .map(|c| ast_type_to_type_name(db, c, module))
                .collect(),
        })
        .collect()
}

fn convert_type_kind(
    db: &dyn TypeCheckDatabase,
    kind: &TypeDefinitionKind<super::refs::CheckerRefs>,
    module: &DefinitionPath,
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
                    type_name: ast_type_to_type_name(db, &f.type_name, module),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, generic_parameters, module),
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
                    type_name: ast_type_to_type_name(db, &p.type_name, module),
                    source_ref: p.source_ref.clone(),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, generic_parameters, module),
            return_type: ast_type_to_type_name(db, return_type, module),
        },
        TypeDefinitionKind::Signature { entries } => TypeDefinitionKind::Signature {
            entries: entries.clone(),
        },
        TypeDefinitionKind::Trait { functions, .. } => TypeDefinitionKind::Trait {
            functions: functions.clone(),
            witness_ref: NoWitness,
        },
        TypeDefinitionKind::Primitive => TypeDefinitionKind::Primitive,
        TypeDefinitionKind::Native {
            generic_parameters,
            factory,
        } => TypeDefinitionKind::Native {
            generic_parameters: convert_generic_params(db, generic_parameters, module),
            factory: factory.clone(),
        },
    }
}

fn get_module_header_params<'db>(
    db: &'db dyn TypeCheckDatabase,
    module: InternedModuleName<'db>,
) -> Vec<(String, DefinitionPath)> {
    collect_module_params_for_path(db, &module.name(db))
}

fn collect_module_params_for_path(
    db: &dyn TypeCheckDatabase,
    module_name: &DefinitionPath,
) -> Vec<(String, DefinitionPath)> {
    let module_def = match db
        .symbol_tables()
        .modules(db)
        .get()
        .get(module_name)
        .cloned()
    {
        Some(d) => d,
        None => return vec![],
    };

    let mut params = if let Some(parent) = &module_def.parent_module {
        collect_module_params_for_path(db, parent)
    } else {
        vec![]
    };

    let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
        return params;
    };
    for def in &ast_module.definitions {
        if let Definition::ModuleHeader {
            params: header_params,
            ..
        } = def
        {
            let own: Vec<(String, DefinitionPath)> = header_params
                .iter()
                .filter_map(|p| {
                    let path = type_resolver::resolve_absolute_path(db, p.path.clone())?;
                    Some((p.name.clone(), path))
                })
                .collect();
            params.extend(own);
            break;
        }
    }
    params
}

pub fn elaborate_function_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    name: InternedFunctionName<'db>,
    program: ProgramInput,
) -> Option<ArcPtr<typed_ast::Function>> {
    let fn_def_ptr = lookup_function_def(db, name)?;
    let fn_def = fn_def_ptr.get();
    match &fn_def.ast_ref {
        CheckerAstRef::Function(arc_fn, _) => {
            let module_name = fn_def.name.module_prefix();
            let interned_module = InternedModuleName::new(db, module_name.clone());
            let module_params = get_module_header_params(db, interned_module);
            let ctx = super::CheckContext {
                file_id: fn_def.source_ref.0,
                module_name: &module_name,
                program,
            };
            Some(ArcPtr::new(super::elaboration::elaborate_function(
                db,
                arc_fn,
                &ctx,
                None,
                &module_params,
            )?))
        }
        CheckerAstRef::ImplFunction(arc_fn, type_name_str, _) => {
            let self_type =
                DefinitionPath::for_type(fn_def.name.module_prefix(), type_name_str.clone());
            let ctx = super::CheckContext {
                file_id: fn_def.source_ref.0,
                module_name: &fn_def.name.module_prefix(),
                program,
            };
            Some(ArcPtr::new(super::elaboration::elaborate_function(
                db,
                arc_fn,
                &ctx,
                Some(self_type),
                &[],
            )?))
        }
        _ => None,
    }
}

#[salsa::tracked]
pub fn elaborate_metadata(
    db: &dyn TypeCheckDatabase,
    program: ProgramInput,
) -> ArcPtr<MetaData<TypedRefs>> {
    let mut typed_metadata: MetaData<TypedRefs> = MetaData::default();

    for fn_def in db.symbol_tables().functions(db).get().values() {
        let interned_name = InternedFunctionName::new(db, fn_def.name.clone());
        let typed_ast_ref = match &fn_def.ast_ref {
            CheckerAstRef::Function(_, kind) => {
                match elaborate_function_def(db, interned_name, program) {
                    Some(ptr) => TypedCheckerAstRef::Function(Arc::clone(&ptr.0), kind.clone()),
                    None => TypedCheckerAstRef::Other(fn_def.ast_ref.clone()),
                }
            }
            CheckerAstRef::ImplFunction(_, type_name_str, kind) => {
                match elaborate_function_def(db, interned_name, program) {
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

    for type_def in db.symbol_tables().types(db).get().values() {
        let kind = convert_type_kind(db, &type_def.kind, &type_def.name.module_prefix());
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

    for (impl_key, impl_def) in db.symbol_tables().impls(db).get() {
        let new_def = ImplDefinition {
            key: impl_def.key.clone(),
            module: impl_def.module.clone(),
            type_name: DefinitionPath::for_type(
                impl_def.module.clone(),
                impl_def.type_name.name().to_string(),
            ),
            trait_name: impl_def
                .trait_name
                .as_ref()
                .map(|t| DefinitionPath::for_type(impl_def.module.clone(), t.name().to_string())),
            source_ref: SourceLocation(impl_def.source_ref.0, impl_def.source_ref.1),
            ast_ref: TypedCheckerAstRef::Other(impl_def.ast_ref.clone()),
        };
        typed_metadata
            .impls
            .insert(impl_key.clone(), Arc::new(new_def));
    }

    for (module_key, module_def) in db.symbol_tables().modules(db).get() {
        let new_def = ModuleDefinition {
            name: module_def.name.clone(),
            visibility: module_def.visibility.clone(),
            is_entry: module_def.is_entry,
            exports: module_def.exports.clone(),
            source_ref: SourceLocation(module_def.source_ref.0, module_def.source_ref.1),
            ast_ref: TypedCheckerAstRef::Other(module_def.ast_ref.clone()),
            parent_module: module_def.parent_module.clone(),
        };
        typed_metadata
            .modules
            .insert(module_key.clone(), Arc::new(new_def));
    }

    ArcPtr::new(typed_metadata)
}
