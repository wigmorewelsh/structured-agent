use super::TypeChecker;
use super::db::{SymbolTablesInput, TypeCheckDatabase};
use super::refs::CheckerAstRef;
use crate::ast::{Type as AstType, TypeParam};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span};
use std::collections::HashMap;
use structured_agent_runtime::symbols::{ModuleName, UseImport};

pub(super) fn resolve_type(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    t: &AstType,
    module: &ModuleName,
    type_imports: &HashMap<String, UseImport>,
) -> AstType {
    match t {
        AstType::Generic(name)
            if tables
                .types(db)
                .get()
                .get(&TypeChecker::resolve_named_type(name, module, type_imports))
                .map(|td| matches!(td.ast_ref, CheckerAstRef::Struct(_)))
                .unwrap_or(false) =>
        {
            AstType::Struct(name.clone())
        }
        AstType::List(inner) => AstType::List(Box::new(resolve_type(
            db,
            tables,
            inner,
            module,
            type_imports,
        ))),
        AstType::Option(inner) => AstType::Option(Box::new(resolve_type(
            db,
            tables,
            inner,
            module,
            type_imports,
        ))),
        other => other.clone(),
    }
}

pub(super) fn validate_type_with_params(
    db: &dyn TypeCheckDatabase,
    tables: SymbolTablesInput,
    ast_type: &AstType,
    span: Span,
    file_id: FileId,
    type_params: &[TypeParam],
    module: &ModuleName,
    type_imports: &HashMap<String, UseImport>,
) -> Result<(), TypeError> {
    match ast_type {
        AstType::Unit | AstType::Boolean | AstType::String | AstType::Int => Ok(()),
        AstType::Generic(name) => {
            if name == "Self" || type_params.iter().any(|tp| tp.name == *name) {
                Ok(())
            } else {
                Err(TypeError::UnboundTypeParameter {
                    name: name.clone(),
                    span,
                    file_id,
                })
            }
        }
        AstType::List(inner) | AstType::Option(inner) => validate_type_with_params(
            db,
            tables,
            inner,
            span,
            file_id,
            type_params,
            module,
            type_imports,
        ),
        AstType::Struct(name) => {
            if tables
                .types(db)
                .get()
                .get(&TypeChecker::resolve_named_type(name, module, type_imports))
                .map(|td| matches!(td.ast_ref, CheckerAstRef::Struct(_)))
                .unwrap_or(false)
            {
                Ok(())
            } else {
                Err(TypeError::UnsupportedType {
                    type_name: name.clone(),
                    span,
                    file_id,
                })
            }
        }
    }
}

impl TypeChecker {
    pub(super) fn resolve_type(
        &self,
        t: &AstType,
        module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> AstType {
        resolve_type(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            t,
            module,
            type_imports,
        )
    }

    pub(super) fn validate_type_with_params(
        &self,
        ast_type: &AstType,
        span: Span,
        file_id: FileId,
        type_params: &[TypeParam],
        module: &ModuleName,
        type_imports: &HashMap<String, UseImport>,
    ) -> Result<(), TypeError> {
        validate_type_with_params(
            &self.db,
            self.symbol_tables.expect("symbol tables not populated"),
            ast_type,
            span,
            file_id,
            type_params,
            module,
            type_imports,
        )
    }

    pub(super) fn unify_type(
        formal: &AstType,
        actual: &AstType,
        subst: &mut HashMap<String, AstType>,
    ) -> bool {
        match formal {
            AstType::Generic(name) => {
                if let Some(bound) = subst.get(name) {
                    bound == actual
                } else {
                    subst.insert(name.clone(), actual.clone());
                    true
                }
            }
            AstType::List(inner_formal) => {
                if let AstType::List(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            AstType::Option(inner_formal) => {
                if let AstType::Option(inner_actual) = actual {
                    Self::unify_type(inner_formal, inner_actual, subst)
                } else {
                    false
                }
            }
            _ => formal == actual,
        }
    }

    pub(super) fn apply_subst(ty: &AstType, subst: &HashMap<String, AstType>) -> AstType {
        match ty {
            AstType::Generic(name) => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
            AstType::List(inner) => AstType::List(Box::new(Self::apply_subst(inner, subst))),
            AstType::Option(inner) => AstType::Option(Box::new(Self::apply_subst(inner, subst))),
            other => other.clone(),
        }
    }
}
