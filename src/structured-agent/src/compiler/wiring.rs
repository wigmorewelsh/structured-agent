use std::collections::HashMap;

use crate::ast::{Definition, ModuleParam};
use crate::typecheck::checker::{FunctionKind, TypedRefs};
use structured_agent_runtime::symbols::{
    FunctionName, FunctionNameKind, MetaData, ModuleName, SymbolQuery, TraitName, TypeName,
};

use crate::ast::ParsedModule;

pub(crate) fn header_params(parsed: &ParsedModule) -> Option<&Vec<ModuleParam>> {
    parsed.module.definitions.iter().find_map(|def| {
        if let Definition::ModuleHeader { params, .. } = def {
            Some(params)
        } else {
            None
        }
    })
}

pub(crate) fn lower_typed_module(
    module: &mut crate::typed_ast::Module,
    module_params: &[ModuleParam],
    metadata: &MetaData<TypedRefs>,
    function_kinds: &HashMap<String, FunctionKind>,
) {
    for def in &mut module.definitions {
        match def {
            crate::typed_ast::Definition::Function(f) => {
                lower_statements(
                    &mut f.body.statements,
                    module_params,
                    metadata,
                    function_kinds,
                );
            }
            crate::typed_ast::Definition::TraitImpl { functions, .. } => {
                for f in functions {
                    lower_statements(
                        &mut f.body.statements,
                        module_params,
                        metadata,
                        function_kinds,
                    );
                }
            }
            _ => {}
        }
    }
}

fn lower_statements(
    stmts: &mut Vec<crate::typed_ast::Statement>,
    module_params: &[ModuleParam],
    metadata: &MetaData<TypedRefs>,
    function_kinds: &HashMap<String, FunctionKind>,
) {
    for stmt in stmts {
        match stmt {
            crate::typed_ast::Statement::Injection(e) => {
                lower_expression(e, module_params, metadata, function_kinds)
            }
            crate::typed_ast::Statement::Assignment { expression, .. } => {
                lower_expression(expression, module_params, metadata, function_kinds)
            }
            crate::typed_ast::Statement::VariableAssignment { expression, .. } => {
                lower_expression(expression, module_params, metadata, function_kinds)
            }
            crate::typed_ast::Statement::ExpressionStatement(e) => {
                lower_expression(e, module_params, metadata, function_kinds)
            }
            crate::typed_ast::Statement::Return(e) => {
                lower_expression(e, module_params, metadata, function_kinds)
            }
            crate::typed_ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                lower_expression(condition, module_params, metadata, function_kinds);
                lower_statements(body, module_params, metadata, function_kinds);
                if let Some(eb) = else_body {
                    lower_statements(eb, module_params, metadata, function_kinds);
                }
            }
            crate::typed_ast::Statement::While {
                condition, body, ..
            } => {
                lower_expression(condition, module_params, metadata, function_kinds);
                lower_statements(body, module_params, metadata, function_kinds);
            }
        }
    }
}

fn lower_expression(
    expr: &mut crate::typed_ast::Expression,
    module_params: &[ModuleParam],
    metadata: &MetaData<TypedRefs>,
    function_kinds: &HashMap<String, FunctionKind>,
) {
    match expr {
        crate::typed_ast::Expression::Call {
            resolved,
            kind,
            arguments,
            ..
        } => {
            let module_str = resolved.module.to_string();
            if let Some(param) = module_params.iter().find(|p| p.name == module_str)
                && param.path.len() >= 2
            {
                let sig_module = param.path[0].clone();
                let sig_name = param.path.last().unwrap().clone();
                let type_name = TypeName {
                    name: param.name.clone(),
                    module: ModuleName::from_str("__param__"),
                };
                let trait_name = TraitName {
                    name: sig_name,
                    module: ModuleName::from_str(&sig_module),
                };
                if let Some(impl_def) = metadata.impl_for(&type_name, &trait_name) {
                    let new_name = resolved.name.clone();
                    *resolved = FunctionName {
                        name: new_name,
                        module: impl_def.module.clone(),
                        kind: FunctionNameKind::Function,
                    };
                    if let Some(new_kind) = function_kinds.get(&resolved.name) {
                        *kind = new_kind.clone();
                    }
                }
            }
            for arg in arguments {
                lower_expression(arg, module_params, metadata, function_kinds);
            }
        }
        crate::typed_ast::Expression::StructLiteral { fields, .. } => {
            for (_, e) in fields {
                lower_expression(e, module_params, metadata, function_kinds);
            }
        }
        crate::typed_ast::Expression::FieldAccess { base, .. } => {
            lower_expression(base, module_params, metadata, function_kinds);
        }
        crate::typed_ast::Expression::ListLiteral { elements, .. } => {
            for e in elements {
                lower_expression(e, module_params, metadata, function_kinds);
            }
        }
        crate::typed_ast::Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            lower_expression(condition, module_params, metadata, function_kinds);
            lower_expression(then_expr, module_params, metadata, function_kinds);
            lower_expression(else_expr, module_params, metadata, function_kinds);
        }
        crate::typed_ast::Expression::Select(select, _) => {
            for clause in &mut select.clauses {
                lower_expression(
                    &mut clause.expression_to_run,
                    module_params,
                    metadata,
                    function_kinds,
                );
                lower_expression(
                    &mut clause.expression_next,
                    module_params,
                    metadata,
                    function_kinds,
                );
            }
        }
        crate::typed_ast::Expression::Variable { .. }
        | crate::typed_ast::Expression::StringLiteral { .. }
        | crate::typed_ast::Expression::BooleanLiteral { .. }
        | crate::typed_ast::Expression::IntLiteral { .. }
        | crate::typed_ast::Expression::Placeholder { .. }
        | crate::typed_ast::Expression::UnitLiteral { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ModuleParam, Type as AstType};
    use crate::typecheck::checker::{CheckerAstRef, SourceLocation, TypedCheckerAstRef};
    use crate::types::Span;
    use std::sync::Arc;
    use structured_agent_runtime::symbols::{
        FunctionName, FunctionNameKind, ImplDefinition, ImplKey, MetaData, ModuleName, TraitName,
        TypeName,
    };

    fn dummy_span() -> Span {
        Span::dummy()
    }

    #[test]
    fn test_lower_typed_module_substitutes_call() {
        let span = dummy_span();
        let call = crate::typed_ast::Expression::Call {
            function: "io::read".to_string(),
            resolved: FunctionName {
                name: "read".to_string(),
                module: ModuleName::from_str("io"),
                kind: FunctionNameKind::Function,
            },
            kind: FunctionKind::External,
            arguments: vec![],
            ty: AstType::String,
            span,
        };
        let mut module = crate::typed_ast::Module {
            definitions: vec![crate::typed_ast::Definition::Function(
                crate::typed_ast::Function {
                    name: "run".to_string(),
                    parameters: vec![],
                    return_type: AstType::Unit,
                    body: crate::typed_ast::FunctionBody {
                        statements: vec![crate::typed_ast::Statement::Return(call)],
                        span,
                    },
                    documentation: None,
                    is_pub: true,
                    span,
                },
            )],
            span,
            file_id: 0,
        };

        let mut metadata: MetaData<TypedRefs> = MetaData::default();
        let key = ImplKey {
            type_name: TypeName {
                name: "io".to_string(),
                module: ModuleName::from_str("__param__"),
            },
            trait_name: TraitName {
                name: "Store".to_string(),
                module: ModuleName::from_str("io_module"),
            },
        };
        let impl_def = ImplDefinition {
            key: key.clone(),
            module: ModuleName::from_str("storage"),
            source_ref: SourceLocation(0, Span::dummy()),
            ast_ref: TypedCheckerAstRef::Other(CheckerAstRef::ModuleParamBinding),
        };
        metadata.impls.insert(key, Arc::new(impl_def));

        let params = vec![ModuleParam {
            name: "io".to_string(),
            path: vec!["io_module".to_string(), "Store".to_string()],
            span: dummy_span(),
        }];

        lower_typed_module(&mut module, &params, &metadata, &HashMap::new());

        if let crate::typed_ast::Definition::Function(f) = &module.definitions[0] {
            if let crate::typed_ast::Statement::Return(crate::typed_ast::Expression::Call {
                resolved,
                ..
            }) = &f.body.statements[0]
            {
                assert_eq!(resolved.to_string(), "storage::read");
            } else {
                panic!("expected return with call");
            }
        } else {
            panic!("expected function definition");
        }
    }
}
