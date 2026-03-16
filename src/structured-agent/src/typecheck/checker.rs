use crate::ast::{
    Definition, Expression, Function, Module, Parameter, SelectClause, Statement, Type as AstType,
};
use crate::typecheck::error::TypeError;
use crate::types::{FileId, Span, Spanned};
use std::collections::HashMap;

pub type ModuleVisibility = HashMap<String, bool>;
pub type AliasToQualified = HashMap<String, String>;

#[derive(Debug)]
pub struct TypeChecker {
    function_signatures: HashMap<String, FunctionSignature>,
    struct_definitions: HashMap<String, Vec<(String, AstType)>>,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    parameters: Vec<Parameter>,
    return_type: AstType,
    is_pub: bool,
}

#[derive(Debug, Clone)]
struct TypeEnvironment {
    variables: HashMap<String, (AstType, Span)>,
    parent: Option<Box<TypeEnvironment>>,
}

struct CheckContext<'a> {
    file_id: FileId,
    alias_map: &'a HashMap<String, String>,
    module_visibility: &'a ModuleVisibility,
    alias_to_qualified: &'a AliasToQualified,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            function_signatures: HashMap::new(),
            struct_definitions: HashMap::new(),
        }
    }

    pub fn check_module(&mut self, module: &Module, file_id: FileId) -> Result<(), TypeError> {
        self.check_module_with_external_sigs(
            module,
            file_id,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
        )
    }

    pub fn check_module_with_external_sigs(
        &mut self,
        module: &Module,
        file_id: FileId,
        external_sigs: &HashMap<String, FunctionSignatureTuple>,
        module_visibility: &ModuleVisibility,
        sig_definitions: &HashMap<String, Vec<crate::ast::SigFunction>>,
    ) -> Result<(), TypeError> {
        for (name, (params, ret, is_pub)) in external_sigs {
            self.function_signatures.insert(
                name.clone(),
                FunctionSignature {
                    parameters: params.clone(),
                    return_type: ret.clone(),
                    is_pub: *is_pub,
                },
            );
        }

        self.register_param_sigs(module, sig_definitions, external_sigs);

        self.collect_function_signatures(module, file_id)?;

        let alias_map = Self::build_alias_map(module);
        let alias_to_qualified = Self::build_alias_to_qualified(module, module_visibility);

        let ctx = CheckContext {
            file_id,
            alias_map: &alias_map,
            module_visibility,
            alias_to_qualified: &alias_to_qualified,
        };

        for definition in &module.definitions {
            if let Definition::Function(func) = definition {
                self.check_function(func, &ctx)?;
            }
        }

        Ok(())
    }

    fn register_param_sigs(
        &mut self,
        module: &Module,
        sig_definitions: &HashMap<String, Vec<crate::ast::SigFunction>>,
        external_sigs: &HashMap<String, FunctionSignatureTuple>,
    ) {
        let params = module.definitions.iter().find_map(|def| {
            if let Definition::ModuleHeader { params, .. } = def {
                Some(params)
            } else {
                None
            }
        });

        let params = match params {
            Some(p) => p,
            None => return,
        };

        for param in params {
            if param.path.len() < 2 {
                continue;
            }
            let concrete_module = &param.path[0];
            let sig_name = param.path.last().unwrap();

            let fn_names: Vec<(String, Vec<crate::ast::Parameter>, crate::ast::Type)> =
                if let Some(fns) = sig_definitions.get(sig_name) {
                    fns.iter()
                        .map(|f| (f.name.clone(), f.parameters.clone(), f.return_type.clone()))
                        .collect()
                } else {
                    external_sigs
                        .iter()
                        .filter_map(|(k, (ps, ret, _))| {
                            k.strip_prefix(&format!("{}::", concrete_module))
                                .map(|fn_name| (fn_name.to_string(), ps.clone(), ret.clone()))
                        })
                        .collect()
                };

            for (fn_name, fn_params, ret_type) in fn_names {
                let key = format!("{}::{}", param.name, fn_name);
                self.function_signatures.insert(
                    key,
                    FunctionSignature {
                        parameters: fn_params,
                        return_type: ret_type,
                        is_pub: true,
                    },
                );
            }
        }
    }

    fn collect_function_signatures(
        &mut self,
        module: &Module,
        file_id: FileId,
    ) -> Result<(), TypeError> {
        for definition in &module.definitions {
            if let Definition::Struct(struct_def) = definition {
                let fields = struct_def
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), f.field_type.clone()))
                    .collect();
                self.struct_definitions
                    .insert(struct_def.name.clone(), fields);
            }
        }

        for definition in &module.definitions {
            match definition {
                Definition::Function(func) => {
                    self.validate_type(&func.return_type, func.span, file_id)?;
                    for param in &func.parameters {
                        self.validate_type(&param.param_type, param.span, file_id)?;
                    }
                    self.function_signatures.insert(
                        func.name.clone(),
                        FunctionSignature {
                            parameters: func.parameters.clone(),
                            return_type: func.return_type.clone(),
                            is_pub: func.is_pub,
                        },
                    );
                }
                Definition::ExternalFunction(ext_func) => {
                    self.validate_type(&ext_func.return_type, ext_func.span, file_id)?;
                    for param in &ext_func.parameters {
                        self.validate_type(&param.param_type, param.span, file_id)?;
                    }
                    self.function_signatures.insert(
                        ext_func.name.clone(),
                        FunctionSignature {
                            parameters: ext_func.parameters.clone(),
                            return_type: ext_func.return_type.clone(),
                            is_pub: ext_func.is_pub,
                        },
                    );
                }
                Definition::Struct(_)
                | Definition::Use { .. }
                | Definition::ModuleHeader { .. }
                | Definition::ModuleBinding { .. }
                | Definition::WiringSite { .. }
                | Definition::Signature { .. } => {}
            }
        }
        Ok(())
    }

    fn build_alias_map(module: &Module) -> HashMap<String, String> {
        module
            .definitions
            .iter()
            .filter_map(|def| {
                if let Definition::Use {
                    path,
                    alias: Some(a),
                    ..
                } = def
                {
                    Some((a.clone(), path.last().cloned().unwrap_or_default()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn build_alias_to_qualified(
        module: &Module,
        module_visibility: &ModuleVisibility,
    ) -> AliasToQualified {
        let mut map = AliasToQualified::new();

        for def in &module.definitions {
            let Definition::Use { path, alias, .. } = def else {
                continue;
            };
            if path.len() < 2 {
                continue;
            }

            let dep_module = &path[0];
            let fn_name = path.last().unwrap();
            let qualified = format!("{}::{}", dep_module, fn_name);

            if module_visibility.contains_key(&qualified) {
                let key = alias.clone().unwrap_or_else(|| fn_name.clone());
                map.insert(key, qualified);
            }
        }

        map
    }

    fn validate_type(
        &self,
        ast_type: &AstType,
        span: Span,
        file_id: FileId,
    ) -> Result<(), TypeError> {
        match ast_type {
            AstType::Unit | AstType::Boolean | AstType::String | AstType::Int => Ok(()),
            AstType::List(inner) | AstType::Option(inner) => {
                self.validate_type(inner, span, file_id)
            }
            AstType::Struct(name) => {
                if self.struct_definitions.contains_key(name) {
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

    fn check_function(&self, func: &Function, ctx: &CheckContext) -> Result<(), TypeError> {
        let mut env = TypeEnvironment::new();
        for param in &func.parameters {
            env.declare_variable(param.name.clone(), param.param_type.clone(), param.span);
        }
        for statement in &func.body.statements {
            env = self.check_statement(statement, env, &func.name, ctx)?;
        }
        Ok(())
    }

    fn check_statement(
        &self,
        statement: &Statement,
        mut env: TypeEnvironment,
        function_name: &str,
        ctx: &CheckContext,
    ) -> Result<TypeEnvironment, TypeError> {
        match statement {
            Statement::Injection(expr) => {
                self.check_expression(expr, &env, ctx)?;
                Ok(env)
            }
            Statement::Assignment {
                variable,
                expression,
                ..
            } => {
                let expr_type = self.check_expression(expression, &env, ctx)?;
                env.declare_variable(variable.clone(), expr_type, expression.span());
                Ok(env)
            }
            Statement::VariableAssignment {
                variable,
                expression,
                span,
            } => {
                let expr_type = self.check_expression(expression, &env, ctx)?;
                let (existing_type, declaration_span) = env
                    .lookup_variable_with_span(variable)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: variable.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    })?;

                if expr_type != existing_type {
                    return Err(TypeError::VariableTypeMismatch {
                        variable: variable.clone(),
                        expected: format!("{}", existing_type),
                        found: format!("{}", expr_type),
                        span: expression.span(),
                        declaration_span,
                        file_id: ctx.file_id,
                    });
                }
                Ok(env)
            }
            Statement::ExpressionStatement(expr) => {
                self.check_expression(expr, &env, ctx)?;
                Ok(env)
            }
            Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                self.check_boolean_condition(condition, &env, ctx)?;
                self.check_block(body, env.create_child(), function_name, ctx)?;
                if let Some(else_stmts) = else_body {
                    self.check_block(else_stmts, env.create_child(), function_name, ctx)?;
                }
                Ok(env)
            }
            Statement::While {
                condition, body, ..
            } => {
                self.check_boolean_condition(condition, &env, ctx)?;
                self.check_block(body, env.create_child(), function_name, ctx)?;
                Ok(env)
            }
            Statement::Return(expr) => {
                let return_type = self.check_expression(expr, &env, ctx)?;
                let expected_type = &self
                    .function_signatures
                    .get(function_name)
                    .expect("function signature not found")
                    .return_type;

                if return_type != *expected_type {
                    return Err(TypeError::ReturnTypeMismatch {
                        function: function_name.to_string(),
                        expected: format!("{}", expected_type),
                        found: format!("{}", return_type),
                        span: expr.span(),
                        file_id: ctx.file_id,
                    });
                }
                Ok(env)
            }
        }
    }

    fn check_boolean_condition(
        &self,
        condition: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<(), TypeError> {
        let cond_type = self.check_expression(condition, env, ctx)?;
        if matches!(cond_type, AstType::Boolean) {
            Ok(())
        } else {
            Err(TypeError::TypeMismatch {
                expected: "Boolean".to_string(),
                found: format!("{}", cond_type),
                span: condition.span(),
                file_id: ctx.file_id,
            })
        }
    }

    fn check_block(
        &self,
        stmts: &[Statement],
        mut env: TypeEnvironment,
        function_name: &str,
        ctx: &CheckContext,
    ) -> Result<(), TypeError> {
        for stmt in stmts {
            env = self.check_statement(stmt, env, function_name, ctx)?;
        }
        Ok(())
    }

    fn check_expression(
        &self,
        expression: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        match expression {
            Expression::Call {
                function,
                arguments,
                span,
            } => self.check_call(function, arguments, *span, env, ctx),
            Expression::Variable { name, span } => {
                env.lookup_variable(name)
                    .ok_or_else(|| TypeError::UnknownVariable {
                        name: name.clone(),
                        span: *span,
                        file_id: ctx.file_id,
                    })
            }
            Expression::StringLiteral { .. } => Ok(AstType::String),
            Expression::BooleanLiteral { .. } => Ok(AstType::Boolean),
            Expression::IntLiteral { .. } => Ok(AstType::Int),
            Expression::UnitLiteral { .. } => Ok(AstType::Unit),
            Expression::Placeholder { span } => Err(TypeError::TypeMismatch {
                expected: "concrete type".to_string(),
                found: "placeholder".to_string(),
                span: *span,
                file_id: ctx.file_id,
            }),
            Expression::ListLiteral { elements, span } => {
                self.check_list_literal(elements, *span, env, ctx)
            }
            Expression::Select(select_expr) => {
                self.check_select(&select_expr.clauses, select_expr.span, env, ctx)
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => self.check_if_else_expression(condition, then_expr, else_expr, env, ctx),
            Expression::StructLiteral {
                struct_name,
                fields,
                span,
            } => self.check_struct_literal(struct_name, fields, *span, env, ctx),
            Expression::FieldAccess { base, field, span } => {
                self.check_field_access(base, field, *span, env, ctx)
            }
        }
    }

    fn check_call(
        &self,
        function: &str,
        arguments: &[Expression],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        let resolved = ctx
            .alias_map
            .get(function)
            .map(String::as_str)
            .unwrap_or(function);

        let qualified_for_vis = ctx
            .alias_to_qualified
            .get(function)
            .or_else(|| ctx.alias_to_qualified.get(resolved))
            .map(String::as_str)
            .unwrap_or(resolved);

        self.check_visibility(qualified_for_vis, resolved, span, ctx)?;

        let func_sig =
            self.function_signatures
                .get(resolved)
                .ok_or_else(|| TypeError::UnknownFunction {
                    name: function.to_string(),
                    span,
                    file_id: ctx.file_id,
                })?;

        if arguments.len() != func_sig.parameters.len() {
            return Err(TypeError::ArgumentCountMismatch {
                function: function.to_string(),
                expected: func_sig.parameters.len(),
                found: arguments.len(),
                span,
                file_id: ctx.file_id,
            });
        }

        for (arg, param) in arguments.iter().zip(&func_sig.parameters) {
            if matches!(arg, Expression::Placeholder { .. }) {
                continue;
            }
            let arg_type = self.check_expression(arg, env, ctx)?;
            if arg_type != param.param_type {
                return Err(TypeError::ArgumentTypeMismatch {
                    function: function.to_string(),
                    parameter: param.name.clone(),
                    expected: format!("{}", param.param_type),
                    found: format!("{}", arg_type),
                    span: arg.span(),
                    file_id: ctx.file_id,
                });
            }
        }

        Ok(func_sig.return_type.clone())
    }

    fn check_visibility(
        &self,
        qualified_for_vis: &str,
        resolved: &str,
        span: Span,
        ctx: &CheckContext,
    ) -> Result<(), TypeError> {
        if ctx.module_visibility.is_empty() {
            return Ok(());
        }

        let name_to_check = if qualified_for_vis.contains("::") {
            qualified_for_vis
        } else if resolved.contains("::") {
            resolved
        } else {
            return Ok(());
        };

        let is_visible = ctx
            .module_visibility
            .get(name_to_check)
            .copied()
            .unwrap_or(true);
        if is_visible {
            Ok(())
        } else {
            Err(TypeError::PrivateFunction {
                name: name_to_check.to_string(),
                span,
                file_id: ctx.file_id,
            })
        }
    }

    fn check_list_literal(
        &self,
        elements: &[Expression],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        if elements.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "non-empty list or type annotation".to_string(),
                found: "empty list".to_string(),
                span,
                file_id: ctx.file_id,
            });
        }

        let first_type = self.check_expression(&elements[0], env, ctx)?;

        for elem in elements.iter().skip(1) {
            let elem_type = self.check_expression(elem, env, ctx)?;
            if first_type != elem_type {
                return Err(TypeError::TypeMismatch {
                    expected: format!("{}", first_type),
                    found: format!("{}", elem_type),
                    span: elem.span(),
                    file_id: ctx.file_id,
                });
            }
        }

        Ok(AstType::List(Box::new(first_type)))
    }

    fn check_select(
        &self,
        clauses: &[SelectClause],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        if clauses.is_empty() {
            return Err(TypeError::TypeMismatch {
                expected: "non-empty select".to_string(),
                found: "empty select".to_string(),
                span,
                file_id: ctx.file_id,
            });
        }

        let first = &clauses[0];
        let first_result_type = self.check_expression(&first.expression_to_run, env, ctx)?;
        let mut first_env = env.create_child();
        first_env.declare_variable(
            first.result_variable.clone(),
            first_result_type,
            first.expression_to_run.span(),
        );
        let first_type = self.check_expression(&first.expression_next, &first_env, ctx)?;

        for (i, clause) in clauses.iter().enumerate().skip(1) {
            let result_type = self.check_expression(&clause.expression_to_run, env, ctx)?;
            let mut clause_env = env.create_child();
            clause_env.declare_variable(
                clause.result_variable.clone(),
                result_type,
                clause.expression_to_run.span(),
            );
            let clause_type = self.check_expression(&clause.expression_next, &clause_env, ctx)?;
            if first_type != clause_type {
                return Err(TypeError::SelectBranchTypeMismatch {
                    expected: format!("{}", first_type),
                    found: format!("{}", clause_type),
                    branch_index: i,
                    span: clause.expression_next.span(),
                    first_branch_span: first.expression_next.span(),
                    file_id: ctx.file_id,
                });
            }
        }

        Ok(first_type)
    }

    fn check_if_else_expression(
        &self,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        self.check_boolean_condition(condition, env, ctx)?;

        let then_type = self.check_expression(then_expr, env, ctx)?;
        let else_type = self.check_expression(else_expr, env, ctx)?;

        if then_type != else_type {
            return Err(TypeError::TypeMismatch {
                expected: format!("{}", then_type),
                found: format!("{}", else_type),
                span: else_expr.span(),
                file_id: ctx.file_id,
            });
        }

        Ok(then_type)
    }

    fn check_struct_literal(
        &self,
        struct_name: &str,
        fields: &[(String, Expression)],
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        let definition = self
            .struct_definitions
            .get(struct_name)
            .ok_or_else(|| TypeError::UnsupportedType {
                type_name: struct_name.to_string(),
                span,
                file_id: ctx.file_id,
            })?
            .clone();

        let mut seen = std::collections::HashSet::new();
        for (field_name, value_expr) in fields {
            if !seen.insert(field_name.clone()) {
                return Err(TypeError::DuplicateField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                });
            }

            let declared_type = definition
                .iter()
                .find(|(n, _)| n == field_name)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| TypeError::UnknownField {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                })?;

            let actual_type = self.check_expression(value_expr, env, ctx)?;
            if actual_type != declared_type {
                return Err(TypeError::StructFieldTypeMismatch {
                    struct_name: struct_name.to_string(),
                    field_name: field_name.clone(),
                    expected: format!("{}", declared_type),
                    found: format!("{}", actual_type),
                    span: value_expr.span(),
                    file_id: ctx.file_id,
                });
            }
        }

        for (required_field, _) in &definition {
            if !fields.iter().any(|(n, _)| n == required_field) {
                return Err(TypeError::MissingField {
                    struct_name: struct_name.to_string(),
                    field_name: required_field.clone(),
                    span,
                    file_id: ctx.file_id,
                });
            }
        }

        Ok(AstType::Struct(struct_name.to_string()))
    }

    fn check_field_access(
        &self,
        base: &Expression,
        field: &str,
        span: Span,
        env: &TypeEnvironment,
        ctx: &CheckContext,
    ) -> Result<AstType, TypeError> {
        let base_type = self.check_expression(base, env, ctx)?;
        match base_type {
            AstType::Struct(name) => {
                let definition = self.struct_definitions.get(&name).ok_or_else(|| {
                    TypeError::UnsupportedType {
                        type_name: name.clone(),
                        span,
                        file_id: ctx.file_id,
                    }
                })?;
                definition
                    .iter()
                    .find(|(n, _)| n == field)
                    .map(|(_, t)| t.clone())
                    .ok_or_else(|| TypeError::UnknownField {
                        struct_name: name.clone(),
                        field_name: field.to_string(),
                        span,
                        file_id: ctx.file_id,
                    })
            }
            other => Err(TypeError::TypeMismatch {
                expected: "struct".to_string(),
                found: format!("{}", other),
                span,
                file_id: ctx.file_id,
            }),
        }
    }
}

pub type FunctionSignatureTuple = (Vec<Parameter>, AstType, bool);

impl TypeEnvironment {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            parent: None,
        }
    }

    fn create_child(&self) -> Self {
        Self {
            variables: HashMap::new(),
            parent: Some(Box::new(self.clone())),
        }
    }

    fn declare_variable(&mut self, name: String, var_type: AstType, span: Span) {
        self.variables.insert(name, (var_type, span));
    }

    fn lookup_variable(&self, name: &str) -> Option<AstType> {
        self.variables
            .get(name)
            .map(|(t, _)| t.clone())
            .or_else(|| self.parent.as_ref()?.lookup_variable(name))
    }

    fn lookup_variable_with_span(&self, name: &str) -> Option<(AstType, Span)> {
        self.variables
            .get(name)
            .cloned()
            .or_else(|| self.parent.as_ref()?.lookup_variable_with_span(name))
    }
}
