use structured_agent_il::slot::{Slot, SlotKind};
use structured_agent_il::{
    BytecodeRef, CompiledFunction, Instruction, builder::InstructionBuilder,
};
use structured_agent_typed_ast::BindingId;

use structured_agent_runtime::Parameter;
use structured_agent_runtime::symbols::FunctionKind;
use structured_agent_typed_ast as typed_ast;
use structured_agent_typed_ast::{NoWitness, SourceLocation, TypedCheckerAstRef, TypedRefs};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use structured_agent_runtime::symbols::{
    DefinitionPath, FunctionDefinition, ImplDefinition, MetaData, ModuleDefinition, References,
    TypeDefinition, clone_kind_typenames,
};

#[derive(Clone)]
pub struct BytecodeRefs;

impl References for BytecodeRefs {
    type Source = SourceLocation;
    type Ast = TypedCheckerAstRef;
    type Body = BytecodeRef;
    type Witness = NoWitness;
    type TypeAnnotation = DefinitionPath;
}

pub struct BytecodeCompiler;

struct CompilerCtx<'a> {
    builder: &'a mut InstructionBuilder,
    binding_id_to_slot: &'a HashMap<BindingId, Slot>,
}

impl BytecodeCompiler {
    pub fn new() -> Self {
        Self
    }

    pub fn compile_to_bytecode(
        &self,
        typed_func: &typed_ast::Function,
    ) -> Result<CompiledFunction, String> {
        let mut builder = InstructionBuilder::new();

        let _ret_slot = builder.alloc_slot(SlotKind::ReturnSlot, "$ret");
        let mut binding_id_to_slot: HashMap<BindingId, Slot> = HashMap::new();

        for param in &typed_func.parameters {
            let slot = builder.alloc_slot(SlotKind::ValueParam, &param.name);
            binding_id_to_slot.insert(param.binding_id, slot);
        }

        for (binding_id, name) in collect_binding_ids(&typed_func.body.statements) {
            binding_id_to_slot
                .entry(binding_id)
                .or_insert_with(|| builder.alloc_slot(SlotKind::Local, &name));
        }

        let has_explicit_return = typed_func
            .body
            .statements
            .iter()
            .any(|s| matches!(s, typed_ast::Statement::Return(_)));

        {
            let mut ctx = CompilerCtx {
                builder: &mut builder,
                binding_id_to_slot: &binding_id_to_slot,
            };

            for stmt in &typed_func.body.statements {
                self.compile_statement(&mut ctx, stmt)?;
            }

            if !has_explicit_return {
                let return_temp = ctx.builder.next_temp_slot();
                if typed_func.return_type == structured_agent_runtime::Type::unit() {
                    ctx.builder.emit(Instruction::LdcUnit { dest: return_temp });
                } else {
                    ctx.builder.emit(Instruction::LlmGenerate {
                        dest: return_temp,
                        return_type: typed_func.return_type.clone(),
                    });
                }
                ctx.builder.emit(Instruction::Ret { var: return_temp });
            }
        }

        let (instructions, labels, slot_table) = builder.build()?;

        Ok(CompiledFunction {
            name: DefinitionPath::for_function(DefinitionPath::root(), typed_func.name.clone()),
            module_name: None,
            parameters: typed_func
                .parameters
                .iter()
                .map(|p| Parameter::new(p.name.clone(), p.param_type.clone()))
                .collect(),
            return_type: typed_func.return_type.clone(),
            instructions,
            labels,
            documentation: typed_func.documentation.clone(),
            slot_table,
        })
    }

    fn compile_statement(
        &self,
        ctx: &mut CompilerCtx,
        stmt: &typed_ast::Statement,
    ) -> Result<(), String> {
        match stmt {
            typed_ast::Statement::Injection(expr) => self.compile_injection(ctx, expr),
            typed_ast::Statement::Assignment {
                binding_id,
                expression,
                ..
            } => self.compile_assignment(ctx, binding_id, expression),
            typed_ast::Statement::VariableAssignment {
                binding_id,
                expression,
                ..
            } => self.compile_variable_assignment(ctx, binding_id, expression),
            typed_ast::Statement::ExpressionStatement(expr) => {
                self.compile_expression_statement(ctx, expr)
            }
            typed_ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => self.compile_if_statement(ctx, condition, body, else_body.as_deref()),
            typed_ast::Statement::While {
                condition, body, ..
            } => self.compile_while_statement(ctx, condition, body),
            typed_ast::Statement::Return(expr) => self.compile_return_statement(ctx, expr),
            typed_ast::Statement::Yield { .. } => {
                ctx.builder.emit(Instruction::ActorYield);
                Ok(())
            }
            typed_ast::Statement::ForIn {
                binding_id,
                iterable,
                move_next_fn,
                current_fn,
                body,
                ..
            } => self.compile_for_in_statement(
                ctx,
                binding_id,
                iterable,
                move_next_fn,
                current_fn,
                body,
            ),
        }
    }

    fn compile_injection(
        &self,
        ctx: &mut CompilerCtx,
        expr: &typed_ast::Expression,
    ) -> Result<(), String> {
        let temp_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, expr, temp_slot)?;
        ctx.builder.emit(Instruction::CtxEvent { var: temp_slot });
        Ok(())
    }

    fn compile_assignment(
        &self,
        ctx: &mut CompilerCtx,
        binding_id: &BindingId,
        expression: &typed_ast::Expression,
    ) -> Result<(), String> {
        let dest_slot = *ctx
            .binding_id_to_slot
            .get(binding_id)
            .ok_or_else(|| format!("binding {:?} not found", binding_id))?;
        self.compile_expression(ctx, expression, dest_slot)
    }

    fn compile_variable_assignment(
        &self,
        ctx: &mut CompilerCtx,
        binding_id: &BindingId,
        expression: &typed_ast::Expression,
    ) -> Result<(), String> {
        let dest_slot = *ctx
            .binding_id_to_slot
            .get(binding_id)
            .ok_or_else(|| format!("binding {:?} not found", binding_id))?;
        self.compile_expression(ctx, expression, dest_slot)
    }

    fn compile_expression_statement(
        &self,
        ctx: &mut CompilerCtx,
        expr: &typed_ast::Expression,
    ) -> Result<(), String> {
        let temp_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, expr, temp_slot)
    }

    fn compile_if_statement(
        &self,
        ctx: &mut CompilerCtx,
        condition: &typed_ast::Expression,
        body: &[typed_ast::Statement],
        else_body: Option<&[typed_ast::Statement]>,
    ) -> Result<(), String> {
        let cond_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, condition, cond_slot)?;

        let id = ctx.builder.next_label_id();
        let else_label = format!("else_{}", id);
        let end_label = format!("end_{}", id);

        ctx.builder.emit_brfalse(cond_slot, &else_label);

        ctx.builder.emit(Instruction::CtxChild);
        for stmt in body {
            self.compile_statement(ctx, stmt)?;
        }
        ctx.builder.emit(Instruction::CtxRestore);
        ctx.builder.emit_br(&end_label);

        ctx.builder.emit_label(&else_label);
        if let Some(else_stmts) = else_body {
            ctx.builder.emit(Instruction::CtxChild);
            for stmt in else_stmts {
                self.compile_statement(ctx, stmt)?;
            }
            ctx.builder.emit(Instruction::CtxRestore);
        }

        ctx.builder.emit_label(&end_label);
        ctx.builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_while_statement(
        &self,
        ctx: &mut CompilerCtx,
        condition: &typed_ast::Expression,
        body: &[typed_ast::Statement],
    ) -> Result<(), String> {
        let id = ctx.builder.next_label_id();
        let loop_start = format!("loop_start_{}", id);
        let loop_end = format!("loop_end_{}", id);

        ctx.builder.emit_label(&loop_start);

        let cond_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, condition, cond_slot)?;
        ctx.builder.emit_brfalse(cond_slot, &loop_end);

        ctx.builder.emit(Instruction::CtxChild);
        for stmt in body {
            self.compile_statement(ctx, stmt)?;
        }
        ctx.builder.emit(Instruction::CtxRestore);
        ctx.builder.emit_br(&loop_start);

        ctx.builder.emit_label(&loop_end);
        ctx.builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_for_in_statement(
        &self,
        ctx: &mut CompilerCtx,
        binding_id: &BindingId,
        iterable: &typed_ast::Expression,
        move_next_fn: &DefinitionPath,
        current_fn: &DefinitionPath,
        body: &[typed_ast::Statement],
    ) -> Result<(), String> {
        let id = ctx.builder.next_label_id();
        let loop_start = format!("for_start_{}", id);
        let loop_end = format!("for_end_{}", id);

        let iter_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, iterable, iter_slot)?;

        ctx.builder.emit(Instruction::CtxChild);
        ctx.builder.emit_label(&loop_start);

        let move_next_slot = ctx.builder.next_temp_slot();
        ctx.builder.emit(Instruction::CallBytecode {
            function_name: move_next_fn.clone(),
            params: vec![iter_slot],
            dest: move_next_slot,
        });
        ctx.builder.emit_brfalse(move_next_slot, &loop_end);

        let x_slot = *ctx
            .binding_id_to_slot
            .get(binding_id)
            .ok_or_else(|| format!("loop variable binding {:?} not found", binding_id))?;
        ctx.builder.emit(Instruction::CallBytecode {
            function_name: current_fn.clone(),
            params: vec![iter_slot],
            dest: x_slot,
        });

        for stmt in body {
            self.compile_statement(ctx, stmt)?;
        }

        ctx.builder.emit_br(&loop_start);
        ctx.builder.emit_label(&loop_end);
        ctx.builder.emit(Instruction::CtxRestore);
        ctx.builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_return_statement(
        &self,
        ctx: &mut CompilerCtx,
        expr: &typed_ast::Expression,
    ) -> Result<(), String> {
        let return_temp = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, expr, return_temp)?;
        ctx.builder.emit(Instruction::Ret { var: return_temp });
        Ok(())
    }

    fn compile_expression(
        &self,
        ctx: &mut CompilerCtx,
        expr: &typed_ast::Expression,
        dest_var: Slot,
    ) -> Result<(), String> {
        match expr {
            typed_ast::Expression::Call {
                binding,
                kind,
                arguments,
                target,
                ..
            } => self.compile_call_expression(
                ctx,
                binding,
                kind.clone(),
                arguments,
                target,
                dest_var,
            ),
            typed_ast::Expression::TypeLiteral { ty, .. } => {
                Self::compile_type_literal(ctx, ty, dest_var);
                Ok(())
            }
            typed_ast::Expression::ModuleInstance { path, .. } => {
                Self::compile_module_instance(ctx, path, dest_var)
            }
            typed_ast::Expression::Variable { binding_id, .. } => {
                Self::compile_variable_expression(ctx, binding_id, dest_var)
            }
            typed_ast::Expression::StringLiteral { value, .. } => {
                Self::compile_string_literal(ctx, value, dest_var)
            }
            typed_ast::Expression::BooleanLiteral { value, .. } => {
                Self::compile_boolean_literal(ctx, *value, dest_var)
            }
            typed_ast::Expression::IntLiteral { value, .. } => {
                Self::compile_int_literal(ctx, *value, dest_var)
            }
            typed_ast::Expression::ListLiteral { elements, .. } => {
                self.compile_list_literal(ctx, elements, dest_var)
            }
            typed_ast::Expression::Placeholder { ty, .. } => {
                Self::compile_placeholder(ctx, dest_var, ty)
            }
            typed_ast::Expression::UnitLiteral { .. } => Self::compile_unit_literal(ctx, dest_var),
            typed_ast::Expression::Select(select, _ty) => {
                self.compile_select_expression(ctx, select, dest_var)
            }
            typed_ast::Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => self.compile_if_else_expression(ctx, condition, then_expr, else_expr, dest_var),
            typed_ast::Expression::StructLiteral {
                struct_name,
                fields,
                ..
            } => self.compile_struct_literal(ctx, struct_name, fields, dest_var),
            typed_ast::Expression::FieldAccess { base, field, .. } => {
                self.compile_field_access(ctx, base, field, dest_var)
            }
            typed_ast::Expression::Spawn { key, ty, .. } => {
                self.compile_spawn_expression(ctx, key, ty, dest_var)
            }
            typed_ast::Expression::StringTemplate { parts, .. } => {
                let mut part_slots = Vec::new();
                for part in parts {
                    let slot = ctx.builder.next_temp_slot();
                    match part {
                        typed_ast::StringPart::Literal(s) => {
                            Self::compile_string_literal(ctx, s, slot)?;
                        }
                        typed_ast::StringPart::Interpolated(expr) => {
                            self.compile_expression(ctx, expr, slot)?;
                        }
                    }
                    part_slots.push(slot);
                }
                ctx.builder.emit(Instruction::StrConcat {
                    dest: dest_var,
                    parts: part_slots,
                });
                Ok(())
            }
        }
    }

    fn compile_call_expression(
        &self,
        ctx: &mut CompilerCtx,
        binding: &typed_ast::MethodBinding,
        kind: FunctionKind,
        arguments: &[typed_ast::Expression],
        target: &Option<Box<typed_ast::Expression>>,
        dest_var: Slot,
    ) -> Result<(), String> {
        let mut params: Vec<Slot> = Vec::new();
        for arg_expr in arguments {
            let temp = ctx.builder.next_temp_slot();
            self.compile_expression(ctx, arg_expr, temp)?;
            params.push(temp);
        }

        match binding {
            typed_ast::MethodBinding::Late(binding_id, fn_path) => {
                let module_slot = *ctx
                    .binding_id_to_slot
                    .get(binding_id)
                    .ok_or_else(|| format!("module param slot not found: {:?}", binding_id))?;
                ctx.builder.emit(Instruction::CallVirtual {
                    module_slot,
                    method: fn_path.last_name().to_string(),
                    params,
                    dest: dest_var,
                });
            }
            typed_ast::MethodBinding::Early(fn_path) => {
                if let Some(actor_expr) = target {
                    let actor_slot = ctx.builder.next_temp_slot();
                    self.compile_expression(ctx, actor_expr, actor_slot)?;
                    ctx.builder.emit(Instruction::CallActor {
                        actor_slot,
                        fn_name: fn_path.clone(),
                        params,
                        dest: dest_var,
                    });
                } else {
                    match kind {
                        FunctionKind::Bytecode => {
                            ctx.builder.emit(Instruction::CallBytecode {
                                function_name: fn_path.clone(),
                                params,
                                dest: dest_var,
                            });
                        }
                        FunctionKind::External => {
                            ctx.builder.emit(Instruction::CallExternal {
                                function_name: fn_path.clone(),
                                params,
                                dest: dest_var,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn compile_spawn_expression(
        &self,
        ctx: &mut CompilerCtx,
        key: &typed_ast::Expression,
        ty: &structured_agent_runtime::Type,
        dest_var: Slot,
    ) -> Result<(), String> {
        let key_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, key, key_slot)?;
        let module_path = ty
            .actor_ref_inner()
            .and_then(|t| match t {
                structured_agent_runtime::Type::Named(p)
                | structured_agent_runtime::Type::Parameterized(p, _) => Some(p.clone()),
                _ => None,
            })
            .ok_or_else(|| "Spawn: expected ActorRef type".to_string())?;
        ctx.builder.emit(Instruction::Spawn {
            module_path,
            key_slot,
            dest: dest_var,
        });
        Ok(())
    }

    fn compile_type_literal(
        ctx: &mut CompilerCtx,
        ty: &structured_agent_runtime::Type,
        dest_var: Slot,
    ) {
        match ty {
            structured_agent_runtime::Type::Named(path)
            | structured_agent_runtime::Type::Parameterized(path, _) => {
                ctx.builder.emit(Instruction::LoadModule {
                    name: path.clone(),
                    dest: dest_var,
                });
            }
            structured_agent_runtime::Type::Generic(_) => {
                ctx.builder.emit(Instruction::LdcUnit { dest: dest_var });
            }
        }
    }

    fn compile_module_instance(
        ctx: &mut CompilerCtx,
        path: &DefinitionPath,
        dest_var: Slot,
    ) -> Result<(), String> {
        ctx.builder.emit(Instruction::LoadModule {
            name: path.clone(),
            dest: dest_var,
        });
        Ok(())
    }

    fn compile_variable_expression(
        ctx: &mut CompilerCtx,
        binding_id: &BindingId,
        dest_var: Slot,
    ) -> Result<(), String> {
        let src = *ctx
            .binding_id_to_slot
            .get(binding_id)
            .ok_or_else(|| format!("binding {:?} not found", binding_id))?;
        ctx.builder.emit(Instruction::Mov {
            dest: dest_var,
            src,
        });
        Ok(())
    }

    fn compile_string_literal(
        ctx: &mut CompilerCtx,
        value: &str,
        dest_var: Slot,
    ) -> Result<(), String> {
        ctx.builder.emit(Instruction::LdcStr {
            dest: dest_var,
            value: value.to_string(),
        });
        Ok(())
    }

    fn compile_boolean_literal(
        ctx: &mut CompilerCtx,
        value: bool,
        dest_var: Slot,
    ) -> Result<(), String> {
        ctx.builder.emit(Instruction::LdcBool {
            dest: dest_var,
            value,
        });
        Ok(())
    }

    fn compile_int_literal(
        ctx: &mut CompilerCtx,
        value: i64,
        dest_var: Slot,
    ) -> Result<(), String> {
        ctx.builder.emit(Instruction::LdcInt {
            dest: dest_var,
            value,
        });
        Ok(())
    }

    fn compile_unit_literal(ctx: &mut CompilerCtx, dest_var: Slot) -> Result<(), String> {
        ctx.builder.emit(Instruction::LdcUnit { dest: dest_var });
        Ok(())
    }

    fn compile_list_literal(
        &self,
        ctx: &mut CompilerCtx,
        elements: &[typed_ast::Expression],
        dest_var: Slot,
    ) -> Result<(), String> {
        let mut element_slots = Vec::new();
        for elem in elements {
            let temp = ctx.builder.next_temp_slot();
            self.compile_expression(ctx, elem, temp)?;
            element_slots.push(temp);
        }
        ctx.builder.emit(Instruction::ListCreate {
            dest: dest_var,
            elements: element_slots,
        });
        Ok(())
    }

    fn compile_placeholder(
        ctx: &mut CompilerCtx,
        dest_var: Slot,
        ty: &structured_agent_runtime::Type,
    ) -> Result<(), String> {
        ctx.builder.emit(Instruction::LlmPlaceholder {
            dest: dest_var,
            param_name: "placeholder".to_string(),
            param_type: ty.clone(),
        });
        Ok(())
    }

    fn compile_select_expression(
        &self,
        ctx: &mut CompilerCtx,
        select: &typed_ast::SelectExpression,
        dest_var: Slot,
    ) -> Result<(), String> {
        let select_id = ctx.builder.next_label_id();
        let select_start = format!("select_start_{}", select_id);
        ctx.builder.emit_label(&select_start);

        let mut clause_labels = Vec::new();
        let mut metadata_slots = Vec::new();

        for i in 0..select.clauses.len() {
            let label_id = ctx.builder.next_label_id();
            let label = format!("clause_{}_{}", i, label_id);
            clause_labels.push(label.clone());

            let function_name = if let typed_ast::Expression::Call { binding, .. } =
                &select.clauses[i].expression_to_run
            {
                match binding {
                    typed_ast::MethodBinding::Early(path) => path.clone(),
                    typed_ast::MethodBinding::Late(_, path) => path.clone(),
                }
            } else {
                return Err(format!("select clause {} expression is not a Call", i));
            };

            let meta_slot = ctx.builder.next_temp_slot();
            ctx.builder.emit(Instruction::MetaFunction {
                function_name,
                dest: meta_slot,
            });
            metadata_slots.push(meta_slot);
        }

        let choice_slot = ctx.builder.next_temp_slot();
        ctx.builder.emit(Instruction::LlmSelect {
            metadata_vars: metadata_slots,
            dest: choice_slot,
        });

        ctx.builder.emit_switch(choice_slot, clause_labels.clone());

        let end_id = ctx.builder.next_label_id();
        let end_label = format!("select_end_{}", end_id);

        for (i, clause) in select.clauses.iter().enumerate() {
            ctx.builder.emit_label(&clause_labels[i]);

            self.compile_expression(ctx, &clause.expression_to_run, dest_var)?;

            ctx.builder.emit_br(&end_label);
        }

        ctx.builder.emit_label(&end_label);
        ctx.builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_if_else_expression(
        &self,
        ctx: &mut CompilerCtx,
        condition: &typed_ast::Expression,
        then_expr: &typed_ast::Expression,
        else_expr: &typed_ast::Expression,
        dest_var: Slot,
    ) -> Result<(), String> {
        let cond_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, condition, cond_slot)?;

        let id = ctx.builder.next_label_id();
        let else_label = format!("ifelse_else_{}", id);
        let end_label = format!("ifelse_end_{}", id);

        ctx.builder.emit_brfalse(cond_slot, &else_label);

        self.compile_expression(ctx, then_expr, dest_var)?;
        ctx.builder.emit_br(&end_label);

        ctx.builder.emit_label(&else_label);
        self.compile_expression(ctx, else_expr, dest_var)?;

        ctx.builder.emit_label(&end_label);
        ctx.builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_struct_literal(
        &self,
        ctx: &mut CompilerCtx,
        struct_name: &str,
        fields: &[(String, typed_ast::Expression)],
        dest_var: Slot,
    ) -> Result<(), String> {
        let mut field_slots = Vec::new();
        for (field_name, field_expr) in fields {
            let temp = ctx.builder.next_temp_slot();
            self.compile_expression(ctx, field_expr, temp)?;
            field_slots.push((field_name.clone(), temp));
        }
        ctx.builder.emit(Instruction::StructNew {
            dest: dest_var,
            struct_name: struct_name.to_string(),
            fields: field_slots,
        });
        Ok(())
    }

    fn compile_field_access(
        &self,
        ctx: &mut CompilerCtx,
        base: &typed_ast::Expression,
        field: &str,
        dest_var: Slot,
    ) -> Result<(), String> {
        let base_slot = ctx.builder.next_temp_slot();
        self.compile_expression(ctx, base, base_slot)?;
        ctx.builder.emit(Instruction::StructGet {
            dest: dest_var,
            src: base_slot,
            field: field.to_string(),
        });
        Ok(())
    }
}

impl Default for BytecodeCompiler {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_from_expr(
    expr: &typed_ast::Expression,
    _result: &mut Vec<(BindingId, String)>,
    _seen: &mut HashSet<BindingId>,
) {
    match expr {
        typed_ast::Expression::Select(select, _) => {
            for clause in &select.clauses {
                collect_from_expr(&clause.expression_to_run, _result, _seen);
            }
        }
        typed_ast::Expression::Call { arguments, .. } => {
            for arg in arguments {
                collect_from_expr(arg, _result, _seen);
            }
        }
        typed_ast::Expression::ListLiteral { elements, .. } => {
            for e in elements {
                collect_from_expr(e, _result, _seen);
            }
        }
        typed_ast::Expression::IfElse {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            collect_from_expr(condition, _result, _seen);
            collect_from_expr(then_expr, _result, _seen);
            collect_from_expr(else_expr, _result, _seen);
        }
        typed_ast::Expression::StructLiteral { fields, .. } => {
            for (_, e) in fields {
                collect_from_expr(e, _result, _seen);
            }
        }
        typed_ast::Expression::FieldAccess { base, .. } => {
            collect_from_expr(base, _result, _seen);
        }
        _ => {}
    }
}

fn collect_binding_ids(statements: &[typed_ast::Statement]) -> Vec<(BindingId, String)> {
    let mut result: Vec<(BindingId, String)> = Vec::new();
    let mut seen: HashSet<BindingId> = HashSet::new();
    collect_binding_ids_inner(statements, &mut result, &mut seen);
    result
}

fn collect_binding_ids_inner(
    statements: &[typed_ast::Statement],
    result: &mut Vec<(BindingId, String)>,
    seen: &mut HashSet<BindingId>,
) {
    for stmt in statements {
        match stmt {
            typed_ast::Statement::Assignment {
                binding_id,
                variable,
                expression,
                ..
            } => {
                if seen.insert(*binding_id) {
                    result.push((*binding_id, variable.clone()));
                }
                collect_from_expr(expression, result, seen);
            }
            typed_ast::Statement::VariableAssignment {
                binding_id,
                variable,
                expression,
                ..
            } => {
                if seen.insert(*binding_id) {
                    result.push((*binding_id, variable.clone()));
                }
                collect_from_expr(expression, result, seen);
            }
            typed_ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                collect_from_expr(condition, result, seen);
                collect_binding_ids_inner(body, result, seen);
                if let Some(else_stmts) = else_body {
                    collect_binding_ids_inner(else_stmts, result, seen);
                }
            }
            typed_ast::Statement::While {
                condition, body, ..
            } => {
                collect_from_expr(condition, result, seen);
                collect_binding_ids_inner(body, result, seen);
            }
            typed_ast::Statement::ExpressionStatement(expr) => {
                collect_from_expr(expr, result, seen);
            }
            typed_ast::Statement::Injection(expr) => {
                collect_from_expr(expr, result, seen);
            }
            typed_ast::Statement::Return(expr) => {
                collect_from_expr(expr, result, seen);
            }
            typed_ast::Statement::Yield { .. } => {}
            typed_ast::Statement::ForIn {
                variable,
                binding_id,
                iterable,
                body,
                ..
            } => {
                collect_from_expr(iterable, result, seen);
                if seen.insert(*binding_id) {
                    result.push((*binding_id, variable.clone()));
                }
                collect_binding_ids_inner(body, result, seen);
            }
        }
    }
}

pub fn compile_metadata(
    typed_metadata: MetaData<TypedRefs>,
) -> Result<MetaData<BytecodeRefs>, String> {
    let compiler = BytecodeCompiler::new();
    let mut new_metadata: MetaData<BytecodeRefs> = MetaData::default();

    for (name, arc_def) in &typed_metadata.functions {
        let body_ref = match &arc_def.ast_ref {
            TypedCheckerAstRef::Function(f, FunctionKind::Bytecode) => {
                let compiled = compiler.compile_to_bytecode(f)?;
                Some(BytecodeRef {
                    instructions: compiled.instructions,
                    labels: compiled.labels,
                    parameters: compiled.parameters,
                    return_type: compiled.return_type,
                    documentation: compiled.documentation,
                    slot_table: compiled.slot_table,
                })
            }
            TypedCheckerAstRef::ImplFunction(f, _, FunctionKind::Bytecode) => {
                let compiled = compiler.compile_to_bytecode(f)?;
                Some(BytecodeRef {
                    instructions: compiled.instructions,
                    labels: compiled.labels,
                    parameters: compiled.parameters,
                    return_type: compiled.return_type,
                    documentation: compiled.documentation,
                    slot_table: compiled.slot_table,
                })
            }
            _ => None,
        };
        new_metadata.functions.insert(
            name.clone(),
            Arc::new(FunctionDefinition {
                name: name.clone(),
                visibility: arc_def.visibility.clone(),
                type_name: arc_def.type_name.clone(),
                source_ref: arc_def.source_ref.clone(),
                ast_ref: arc_def.ast_ref.clone(),
                body_ref,
            }),
        );
    }

    for (name, arc_def) in &typed_metadata.modules {
        new_metadata.modules.insert(
            name.clone(),
            Arc::new(ModuleDefinition {
                name: arc_def.name.clone(),
                visibility: arc_def.visibility.clone(),
                is_entry: arc_def.is_entry,
                exports: arc_def.exports.clone(),
                source_ref: arc_def.source_ref.clone(),
                ast_ref: arc_def.ast_ref.clone(),
                parent_module: arc_def.parent_module.clone(),
            }),
        );
    }

    for (name, arc_def) in &typed_metadata.types {
        new_metadata.types.insert(
            name.clone(),
            Arc::new(TypeDefinition {
                name: arc_def.name.clone(),
                kind: clone_kind_typenames(&arc_def.kind),
                source_ref: arc_def.source_ref.clone(),
                ast_ref: arc_def.ast_ref.clone(),
            }),
        );
    }

    for (key, arc_def) in &typed_metadata.impls {
        new_metadata.impls.insert(
            key.clone(),
            Arc::new(ImplDefinition {
                key: arc_def.key.clone(),
                module: arc_def.module.clone(),
                type_name: arc_def.type_name.clone(),
                trait_name: arc_def.trait_name.clone(),
                source_ref: arc_def.source_ref.clone(),
                ast_ref: arc_def.ast_ref.clone(),
            }),
        );
    }

    Ok(new_metadata)
}
