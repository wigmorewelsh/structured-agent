use super::refs::{
    CheckerAstRef, CheckerRefs, FunctionKind, NoWitness, SourceLocation, TypedCheckerAstRef,
    TypedRefs,
};
use crate::ast::{Definition, Module as AstModule, Type as AstType, TypeParam, Use, UseParam};
use crate::typecheck::TypeError;
use crate::typecheck::error::OrAccumulateError;

use crate::typed_ast;
use crate::types::{FileId, Span};
use nonempty::NonEmpty;

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    DefinitionPath, DefinitionSegment, FieldDefinition, FunctionDefinition,
    GenericParameterDefinition, ImplDefinition, MetaData, ModuleDefinition, ParameterDefinition,
    SignatureEntry, TypeDefinition, TypeDefinitionKind,
};

#[salsa::db]
pub trait TypeCheckDatabase: salsa::Database {}

#[salsa::db]
#[derive(Default)]
pub struct TypeCheckDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for TypeCheckDb {}

#[salsa::db]
impl TypeCheckDatabase for TypeCheckDb {}

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
pub fn lookup_type_def_in_symbol_tables<'db>(
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
pub fn get_function_sig<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: InternedFunctionName<'db>,
    program: ProgramInput,
) -> Option<ArcPtr<super::FunctionSignature>> {
    let fn_name = name.name(db);
    let fn_def = lookup_function_def(db, tables, name).or_accumulate(
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
    let type_def = lookup_type_def_in_symbol_tables(db, tables, type_key).or_accumulate(
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
    if fn_name.is_impl_fn() {
        type_env.set_self_type(DefinitionPath::for_type(
            fn_name.module_prefix(),
            fn_name.last_name(),
        ));
    };

    let mut resolved_params = Vec::with_capacity(parameters.len());
    for p in parameters {
        let param_ctx = super::CheckContext {
            file_id: p.source_ref.0,
            module_name: &fn_name.module_prefix(),
            program,
        };
        let param_type = super::synthesize::resolve(
            db,
            tables,
            &p.type_name,
            &type_env,
            p.source_ref.1,
            &param_ctx,
        )?;
        resolved_params.push(crate::typed_ast::Parameter {
            name: p.name.clone(),
            param_type,
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
        tables,
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
    tables: SymbolTablesInput,
    name: &str,
    current_module: &DefinitionPath,
) -> Option<(Vec<(String, AstType)>, Vec<TypeParam>)> {
    let interned_mod = InternedModuleName::new(db, current_module.clone());
    let interned_name = name.intern(db);
    let resolved = resolve_type_in_module(db, tables, interned_mod, interned_name)
        .unwrap_or_else(|| DefinitionPath::for_type(current_module.clone(), name));
    let key = InternedTypeName::new(db, resolved);
    lookup_type_def_in_symbol_tables(db, tables, key).and_then(|arc_ptr| {
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
pub fn check_program(db: &dyn TypeCheckDatabase, program: ProgramInput, tables: SymbolTablesInput) {
    for parsed in program.modules(db) {
        check_module(db, parsed, tables, program);
    }
}

#[salsa::tracked]
pub fn check_module(
    db: &dyn TypeCheckDatabase,
    parsed: ParsedModuleInput,
    tables: SymbolTablesInput,
    program: ProgramInput,
) {
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
        super::synthesize::check_definition(db, tables, def, &ctx);
    }
}

#[salsa::tracked]
pub fn lookup_type_in_symbol_tables<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let key = DefinitionPath::for_type(module.name(db), symbol.value(db));
    tables.types(db).get().get(&key).map(|_| key)
}

// --- new type resolution

type DefKind = TypeDefinitionKind<CheckerRefs>;

fn resolve_absolute_path<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    use_path: NonEmpty<String>,
) -> Option<DefinitionPath> {
    let root = InternedModuleName::new(db, DefinitionPath::root());
    let head_symbol = use_path.head.clone().intern(db);
    let mut last_type_name = lookup_type_in_symbol_tables(db, tables, root, head_symbol);
    let mut search_module = InternedModuleName::new(
        db,
        DefinitionPath::for_module(NonEmpty::new(use_path.head.clone())),
    );
    for symbol in use_path.tail.iter() {
        let type_name =
            resolve_type_in_module(db, tables, search_module, symbol.clone().intern(db))?;
        let type_def = lookup_type_def_in_symbol_tables(
            db,
            tables,
            InternedTypeName::new(db, type_name.clone()),
        )?;
        if let DefKind::Signature { .. } = type_def.get().kind {
            search_module = InternedModuleName::new(db, type_name.clone());
        }
        last_type_name = Some(type_name);
    }
    last_type_name
}

fn resolve_local_use_path<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    use_path: Arc<Use>,
) -> Option<DefinitionPath> {
    let mut search_module = current_module.clone();
    let mut last_type_name = None;
    for seg in use_path.path.iter() {
        let symbol = seg.name.clone().intern(db);
        let type_name = resolve_type_in_module(db, tables, search_module, symbol)?;
        let type_def = lookup_type_def_in_symbol_tables(
            db,
            tables,
            InternedTypeName::new(db, type_name.clone()),
        )?;
        if let DefKind::Signature { .. } = type_def.get().kind {
            search_module = InternedModuleName::new(db, type_name.clone());
        }
        last_type_name = Some(type_name);
    }
    last_type_name
}

fn resolve_type_cycle_recovery<'db>(
    _db: &'db dyn TypeCheckDatabase,
    _id: salsa::Id,
    _tables: SymbolTablesInput,
    _current_module: InternedModuleName<'db>,
    _symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    None
}

#[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
pub fn resolve_type_in_module<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let prelude = InternedModuleName::new(
        db,
        DefinitionPath::for_module(NonEmpty::new("prelude".to_string())),
    );
    let unstable = InternedModuleName::new(
        db,
        DefinitionPath::for_module(NonEmpty::new("unstable".to_string())),
    );
    lookup_type_in_symbol_tables(db, tables, current_module, symbol)
        .or_else(|| resolve_type_as_mod_param(db, tables, current_module, symbol))
        .or_else(|| resolve_type_as_alias(db, tables, current_module, symbol))
        .or_else(|| resolve_type_as_use(db, tables, current_module, symbol))
        .or_else(|| lookup_type_in_symbol_tables(db, tables, prelude, symbol))
        .or_else(|| lookup_type_in_symbol_tables(db, tables, unstable, symbol))
}

#[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
fn resolve_type_as_relative_module<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let key = current_module.name(db).with_module(symbol.value(db));
    tables.types(db).get().get(&key).map(|_| key)
}

#[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
pub fn resolve_type_as_mod_param<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let module_def = tables
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
                    return resolve_absolute_path(db, tables, param.path.clone());
                }
            }
        }
    }

    None
}

#[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
pub fn resolve_type_as_alias<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let module_name = current_module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
    let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
        return None;
    };
    let alias_str = symbol.value(db);
    for def in &ast_module.definitions {
        if let Definition::Use(u) = def
            && let Some(use_alias) = &u.alias
        {
            if use_alias == alias_str.as_str() {
                return resolve_local_use_path(db, tables, current_module, u.clone());
            }
        }
    }
    None
}

#[salsa::tracked(cycle_result = resolve_type_cycle_recovery)]
pub fn resolve_type_as_use<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let module_name = current_module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
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
                return resolve_local_use_path(db, tables, current_module, u.clone());
            }
        }
    }
    None
}

// --- new type resolution -- end

#[salsa::tracked]
pub fn resolve_function_call<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    symbol: InternedString<'db>,
) -> Option<DefinitionPath> {
    let type_name = resolve_type_in_module(db, tables, current_module, symbol)?;
    let type_def =
        lookup_type_def_in_symbol_tables(db, tables, InternedTypeName::new(db, type_name.clone()))?;
    if let DefKind::Function { .. } = type_def.get().kind {
        Some(DefinitionPath::for_function(
            type_name.module_prefix(),
            type_name.last_name(),
        ))
    } else {
        None
    }
}

fn resolve_path_to_module<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    path: &NonEmpty<String>,
) -> Option<DefinitionPath> {
    let mut search = current_module;
    let mut last_type = None;
    for seg in path.iter() {
        let sym = seg.clone().intern(db);
        let type_name = resolve_type_in_module(db, tables, search, sym)?;
        if let Some(type_def) = lookup_type_def_in_symbol_tables(
            db,
            tables,
            InternedTypeName::new(db, type_name.clone()),
        ) {
            if let DefKind::Signature { .. } = type_def.get().kind {
                search = InternedModuleName::new(db, type_name.clone());
            }
        }
        last_type = Some(type_name);
    }
    last_type
}

#[salsa::tracked]
pub fn resolve_use_param_bindings<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    alias: InternedString<'db>,
) -> Vec<(String, DefinitionPath)> {
    resolve_use_param_bindings_inner(db, tables, current_module, alias).unwrap_or_default()
}

fn resolve_use_param_bindings_inner<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    alias: InternedString<'db>,
) -> Option<Vec<(String, DefinitionPath)>> {
    let module_name = current_module.name(db);
    let module_def = tables.modules(db).get().get(&module_name).cloned()?;
    let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
        return None;
    };
    let alias_str = alias.value(db);

    let mut u = None;
    for def in &ast_module.definitions {
        let Definition::Use(candidate) = def else {
            continue;
        };
        let name = candidate.path.last().name.as_str();
        let effective = candidate.alias.as_deref().unwrap_or(name);
        if effective == alias_str.as_str() {
            u = Some(candidate.clone());
            break;
        }
    }
    let u = u?;

    let parameterized_seg = u.path.iter().find(|seg| !seg.params.is_empty())?;

    let func_type = resolve_type_in_module(db, tables, current_module, alias)?;
    let func_module_def = tables
        .modules(db)
        .get()
        .get(&func_type.module_prefix())
        .cloned()?;
    let CheckerAstRef::Module(func_ast) = &func_module_def.ast_ref else {
        return None;
    };
    let mut header_params: Vec<crate::ast::ModuleParam> = vec![];
    for d in &func_ast.definitions {
        if let Definition::ModuleHeader { params, .. } = d {
            header_params = params.clone();
            break;
        }
    }

    let mut result = Vec::new();
    for (i, use_param) in parameterized_seg.params.iter().enumerate() {
        match use_param {
            UseParam::Positional(path_segs) => {
                if let (Some(p), Some(concrete)) = (
                    header_params.get(i),
                    resolve_path_to_module(db, tables, current_module, path_segs),
                ) {
                    result.push((p.name.clone(), concrete));
                }
            }
            UseParam::Named {
                name: param_name,
                path: path_segs,
            } => {
                if let Some(concrete) =
                    resolve_path_to_module(db, tables, current_module, path_segs)
                {
                    result.push((param_name.clone(), concrete));
                }
            }
        }
    }
    Some(result)
}

#[salsa::tracked]
pub fn resolve_function_alias_via_param<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    current_module: InternedModuleName<'db>,
    alias: InternedString<'db>,
) -> Option<String> {
    resolve_function_call(db, tables, current_module, alias)?;
    let module_name = current_module.name(db);
    let module_def = tables.modules(db).get().get(&module_name)?.clone();
    let CheckerAstRef::Module(ast_module) = &module_def.ast_ref else {
        return None;
    };
    let alias_str = alias.value(db);
    let header_params: Vec<crate::ast::ModuleParam> = ast_module
        .definitions
        .iter()
        .find_map(|d| {
            if let Definition::ModuleHeader { params, .. } = d {
                Some(params.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();
    for def in &ast_module.definitions {
        if let Definition::Use(u) = def {
            let name = u.path.last().name.clone();
            let effective = u
                .alias
                .as_ref()
                .map(String::as_str)
                .unwrap_or(name.as_str());
            if effective != alias_str.as_str() {
                continue;
            }
            let seg = u.path.first();
            if header_params.iter().any(|p| p.name == seg.name) {
                return Some(seg.name.clone());
            }
        }
    }
    None
}

pub fn ast_type_to_type_name(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    ty: &AstType,
    module_name: &DefinitionPath,
) -> DefinitionPath {
    let interned_mod = InternedModuleName::new(db, module_name.clone());
    let interned_name = ty.name.clone().intern(db);
    resolve_type_in_module(db, tables, interned_mod, interned_name)
        .unwrap_or_else(|| DefinitionPath::for_type(module_name.clone(), ty.name.clone()))
}

fn convert_generic_params(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
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
                .map(|c| ast_type_to_type_name(db, tables, c, module))
                .collect(),
        })
        .collect()
}

fn convert_type_kind(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
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
                    source_ref: p.source_ref.clone(),
                })
                .collect(),
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
            return_type: ast_type_to_type_name(db, tables, return_type, module),
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
            generic_parameters: convert_generic_params(db, tables, generic_parameters, module),
            factory: factory.clone(),
        },
    }
}

#[salsa::tracked]
pub fn elaborate_function_def<'db>(
    db: &'db dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    name: InternedFunctionName<'db>,
    program: ProgramInput,
) -> Option<ArcPtr<typed_ast::Function>> {
    let fn_def_ptr = lookup_function_def(db, tables, name)?;
    let fn_def = fn_def_ptr.get();
    match &fn_def.ast_ref {
        CheckerAstRef::Function(arc_fn, _) => {
            let ctx = super::CheckContext {
                file_id: fn_def.source_ref.0,
                module_name: &fn_def.name.module_prefix(),
                program,
            };
            Some(ArcPtr::new(super::elaboration::elaborate_function(
                db, tables, arc_fn, &ctx, None,
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
                tables,
                arc_fn,
                &ctx,
                Some(self_type),
            )?))
        }
        _ => None,
    }
}

#[salsa::tracked]
pub fn elaborate_metadata(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    program: ProgramInput,
) -> ArcPtr<MetaData<TypedRefs>> {
    let mut typed_metadata: MetaData<TypedRefs> = MetaData::default();

    for fn_def in tables.functions(db).get().values() {
        let interned_name = InternedFunctionName::new(db, fn_def.name.clone());
        let typed_ast_ref = match &fn_def.ast_ref {
            CheckerAstRef::Function(_, kind) => {
                match elaborate_function_def(db, tables, interned_name, program) {
                    Some(ptr) => TypedCheckerAstRef::Function(Arc::clone(&ptr.0), kind.clone()),
                    None => TypedCheckerAstRef::Other(fn_def.ast_ref.clone()),
                }
            }
            CheckerAstRef::ImplFunction(_, type_name_str, kind) => {
                match elaborate_function_def(db, tables, interned_name, program) {
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
        let kind = convert_type_kind(db, tables, &type_def.kind, &type_def.name.module_prefix());
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
            type_name: DefinitionPath::for_type(
                impl_def.module.clone(),
                impl_def.type_name.name.clone(),
            ),
            trait_name: DefinitionPath::for_type(
                impl_def.module.clone(),
                impl_def.trait_name.name.clone(),
            ),
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
            parent_module: module_def.parent_module.clone(),
        };
        typed_metadata
            .modules
            .insert(module_key.clone(), Arc::new(new_def));
    }

    ArcPtr::new(typed_metadata)
}
