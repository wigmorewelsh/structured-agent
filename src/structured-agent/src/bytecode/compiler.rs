use super::{BytecodeFunctionExpr, Instruction, builder::InstructionBuilder};
use crate::ast;
use crate::typecheck::checker::FunctionKind;
use crate::typed_ast;
use crate::types::{ExecutableFunction, Parameter};
use std::fmt;

#[derive(Clone, Debug)]
pub struct CompiledFunction {
    pub name: String,
    pub module_name: Option<String>,
    pub parameters: Vec<Parameter>,
    pub return_type: crate::types::Type,
    pub instructions: Vec<Instruction>,
    pub labels: std::collections::HashMap<String, usize>,
    pub documentation: Option<String>,
}

pub struct BytecodeCompiler;

impl BytecodeCompiler {
    pub fn new() -> Self {
        Self
    }

    pub fn compile_to_bytecode(
        &self,
        typed_func: &typed_ast::Function,
    ) -> Result<CompiledFunction, String> {
        let mut builder = InstructionBuilder::new();

        let mut has_explicit_return = false;
        for stmt in &typed_func.body.statements {
            if matches!(stmt, typed_ast::Statement::Return(_)) {
                has_explicit_return = true;
            }
            self.compile_statement(&mut builder, stmt)?;
        }

        if !has_explicit_return {
            let return_temp = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: return_temp.clone(),
            });
            if typed_func.return_type == ast::Type::Unit {
                builder.emit(Instruction::LdcUnit {
                    dest: return_temp.clone(),
                });
            } else {
                let return_type_str = Self::type_to_string(&typed_func.return_type);
                builder.emit(Instruction::LlmGenerate {
                    dest: return_temp.clone(),
                    return_type: return_type_str,
                });
            }
            builder.emit(Instruction::Ret { var: return_temp });
        }

        let (instructions, labels) = builder.build()?;

        Ok(CompiledFunction {
            name: typed_func.name.clone(),
            module_name: None,
            parameters: typed_func
                .parameters
                .iter()
                .map(|p| Parameter::new(p.name.clone(), Self::convert_type(&p.param_type)))
                .collect(),
            return_type: Self::convert_type(&typed_func.return_type),
            instructions,
            labels,
            documentation: typed_func.documentation.clone(),
        })
    }

    fn compile_statement(
        &self,
        builder: &mut InstructionBuilder,
        stmt: &typed_ast::Statement,
    ) -> Result<(), String> {
        match stmt {
            typed_ast::Statement::Injection(expr) => self.compile_injection(builder, expr),
            typed_ast::Statement::Assignment {
                variable,
                expression,
                ..
            } => self.compile_assignment(builder, variable, expression),
            typed_ast::Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => self.compile_variable_assignment(builder, variable, expression),
            typed_ast::Statement::ExpressionStatement(expr) => {
                self.compile_expression_statement(builder, expr)
            }
            typed_ast::Statement::If {
                condition,
                body,
                else_body,
                ..
            } => self.compile_if_statement(builder, condition, body, else_body.as_deref()),
            typed_ast::Statement::While {
                condition, body, ..
            } => self.compile_while_statement(builder, condition, body),
            typed_ast::Statement::Return(expr) => self.compile_return_statement(builder, expr),
        }
    }

    fn compile_injection(
        &self,
        builder: &mut InstructionBuilder,
        expr: &typed_ast::Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        self.compile_expression(builder, expr, &temp_var)?;
        builder.emit(Instruction::CtxEvent {
            var: temp_var.clone(),
        });
        builder.emit(Instruction::Drop { name: temp_var });
        Ok(())
    }

    fn compile_assignment(
        &self,
        builder: &mut InstructionBuilder,
        variable: &str,
        expression: &typed_ast::Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        self.compile_expression(builder, expression, &temp_var)?;
        builder.emit(Instruction::Decl {
            name: variable.to_string(),
        });
        builder.emit(Instruction::Mov {
            dest: variable.to_string(),
            src: temp_var.clone(),
        });
        builder.emit(Instruction::Drop { name: temp_var });
        Ok(())
    }

    fn compile_variable_assignment(
        &self,
        builder: &mut InstructionBuilder,
        variable: &str,
        expression: &typed_ast::Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        self.compile_expression(builder, expression, &temp_var)?;
        builder.emit(Instruction::Mov {
            dest: variable.to_string(),
            src: temp_var.clone(),
        });
        builder.emit(Instruction::Drop { name: temp_var });
        Ok(())
    }

    fn compile_expression_statement(
        &self,
        builder: &mut InstructionBuilder,
        expr: &typed_ast::Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        self.compile_expression(builder, expr, &temp_var)?;
        builder.emit(Instruction::Drop { name: temp_var });
        Ok(())
    }

    fn compile_if_statement(
        &self,
        builder: &mut InstructionBuilder,
        condition: &typed_ast::Expression,
        body: &[typed_ast::Statement],
        else_body: Option<&[typed_ast::Statement]>,
    ) -> Result<(), String> {
        let if_start = format!("if_start_{}", builder.next_temp());
        builder.emit_label(&if_start);

        let cond_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: cond_var.clone(),
        });
        self.compile_expression(builder, condition, &cond_var)?;

        let else_label = format!("else_{}", builder.next_temp());
        let end_label = format!("end_{}", builder.next_temp());

        builder.emit_brfalse(cond_var, &else_label);

        builder.emit(Instruction::CtxChild {
            is_scope_boundary: false,
        });
        for stmt in body {
            self.compile_statement(builder, stmt)?;
        }
        builder.emit(Instruction::CtxRestore);
        builder.emit_br(&end_label);

        builder.emit_label(&else_label);
        if let Some(else_stmts) = else_body {
            builder.emit(Instruction::CtxChild {
                is_scope_boundary: false,
            });
            for stmt in else_stmts {
                self.compile_statement(builder, stmt)?;
            }
            builder.emit(Instruction::CtxRestore);
        }

        builder.emit_label(&end_label);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_while_statement(
        &self,
        builder: &mut InstructionBuilder,
        condition: &typed_ast::Expression,
        body: &[typed_ast::Statement],
    ) -> Result<(), String> {
        let loop_start = format!("loop_start_{}", builder.next_temp());
        let loop_end = format!("loop_end_{}", builder.next_temp());

        builder.emit_label(&loop_start);

        let cond_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: cond_var.clone(),
        });
        self.compile_expression(builder, condition, &cond_var)?;
        builder.emit_brfalse(cond_var, &loop_end);

        builder.emit(Instruction::CtxChild {
            is_scope_boundary: false,
        });
        for stmt in body {
            self.compile_statement(builder, stmt)?;
        }
        builder.emit(Instruction::CtxRestore);
        builder.emit_br(&loop_start);

        builder.emit_label(&loop_end);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_return_statement(
        &self,
        builder: &mut InstructionBuilder,
        expr: &typed_ast::Expression,
    ) -> Result<(), String> {
        let return_temp = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: return_temp.clone(),
        });
        self.compile_expression(builder, expr, &return_temp)?;
        builder.emit(Instruction::Ret { var: return_temp });
        Ok(())
    }

    fn compile_expression(
        &self,
        builder: &mut InstructionBuilder,
        expr: &typed_ast::Expression,
        dest_var: &str,
    ) -> Result<(), String> {
        match expr {
            typed_ast::Expression::Call {
                resolved,
                kind,
                arguments,
                ..
            } => self.compile_call_expression(builder, resolved, kind.clone(), arguments, dest_var),
            typed_ast::Expression::Variable { name, .. } => {
                Self::compile_variable_expression(builder, name, dest_var)
            }
            typed_ast::Expression::StringLiteral { value, .. } => {
                Self::compile_string_literal(builder, value, dest_var)
            }
            typed_ast::Expression::BooleanLiteral { value, .. } => {
                Self::compile_boolean_literal(builder, *value, dest_var)
            }
            typed_ast::Expression::IntLiteral { value, .. } => {
                Self::compile_int_literal(builder, *value, dest_var)
            }
            typed_ast::Expression::ListLiteral { elements, .. } => {
                self.compile_list_literal(builder, elements, dest_var)
            }
            typed_ast::Expression::Placeholder { .. } => {
                Self::compile_placeholder(builder, dest_var)
            }
            typed_ast::Expression::UnitLiteral { .. } => {
                Self::compile_unit_literal(builder, dest_var)
            }
            typed_ast::Expression::Select(select, _ty) => {
                self.compile_select_expression(builder, select, dest_var)
            }
            typed_ast::Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.compile_if_else_expression(builder, condition, then_expr, else_expr, dest_var)
            }
            typed_ast::Expression::StructLiteral {
                struct_name,
                fields,
                ..
            } => self.compile_struct_literal(builder, struct_name, fields, dest_var),
            typed_ast::Expression::FieldAccess { base, field, .. } => {
                self.compile_field_access(builder, base, field, dest_var)
            }
        }
    }

    fn compile_call_expression(
        &self,
        builder: &mut InstructionBuilder,
        function: &str,
        kind: FunctionKind,
        arguments: &[typed_ast::Expression],
        dest_var: &str,
    ) -> Result<(), String> {
        let mut params = Vec::new();

        for arg_expr in arguments {
            let temp_var = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_var.clone(),
            });
            self.compile_expression(builder, arg_expr, &temp_var)?;
            params.push(temp_var);
        }

        let instruction = match kind {
            FunctionKind::Bytecode => Instruction::CallBytecode {
                function_name: function.to_string(),
                params,
                dest: dest_var.to_string(),
            },
            FunctionKind::External => Instruction::CallExternal {
                function_name: function.to_string(),
                params,
                dest: dest_var.to_string(),
            },
        };
        builder.emit(instruction);
        Ok(())
    }

    fn compile_variable_expression(
        builder: &mut InstructionBuilder,
        name: &str,
        dest_var: &str,
    ) -> Result<(), String> {
        builder.emit(Instruction::Mov {
            dest: dest_var.to_string(),
            src: name.to_string(),
        });
        Ok(())
    }

    fn compile_string_literal(
        builder: &mut InstructionBuilder,
        value: &str,
        dest_var: &str,
    ) -> Result<(), String> {
        builder.emit(Instruction::LdcStr {
            dest: dest_var.to_string(),
            value: value.to_string(),
        });
        Ok(())
    }

    fn compile_boolean_literal(
        builder: &mut InstructionBuilder,
        value: bool,
        dest_var: &str,
    ) -> Result<(), String> {
        builder.emit(Instruction::LdcBool {
            dest: dest_var.to_string(),
            value,
        });
        Ok(())
    }

    fn compile_int_literal(
        builder: &mut InstructionBuilder,
        value: i64,
        dest_var: &str,
    ) -> Result<(), String> {
        builder.emit(Instruction::LdcInt {
            dest: dest_var.to_string(),
            value,
        });
        Ok(())
    }

    fn compile_unit_literal(
        builder: &mut InstructionBuilder,
        dest_var: &str,
    ) -> Result<(), String> {
        builder.emit(Instruction::LdcUnit {
            dest: dest_var.to_string(),
        });
        Ok(())
    }

    fn compile_list_literal(
        &self,
        builder: &mut InstructionBuilder,
        elements: &[typed_ast::Expression],
        dest_var: &str,
    ) -> Result<(), String> {
        let mut element_vars = Vec::new();
        for elem in elements {
            let temp_var = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_var.clone(),
            });
            self.compile_expression(builder, elem, &temp_var)?;
            element_vars.push(temp_var);
        }
        builder.emit(Instruction::ListCreate {
            dest: dest_var.to_string(),
            elements: element_vars,
        });
        Ok(())
    }

    fn compile_placeholder(builder: &mut InstructionBuilder, dest_var: &str) -> Result<(), String> {
        builder.emit(Instruction::LlmPlaceholder {
            dest: dest_var.to_string(),
            param_name: "placeholder".to_string(),
            param_type: "Unknown".to_string(),
        });
        Ok(())
    }

    fn compile_select_expression(
        &self,
        builder: &mut InstructionBuilder,
        select: &typed_ast::SelectExpression,
        dest_var: &str,
    ) -> Result<(), String> {
        let select_start = format!("select_start_{}", builder.next_temp());
        builder.emit_label(&select_start);

        builder.emit(Instruction::Decl {
            name: dest_var.to_string(),
        });

        let mut clause_labels = Vec::new();
        let mut metadata_vars = Vec::new();

        for i in 0..select.clauses.len() {
            let label = format!("clause_{}_{}", i, builder.next_temp());
            clause_labels.push(label.clone());

            let function_name = if let typed_ast::Expression::Call { function, .. } =
                &select.clauses[i].expression_to_run
            {
                function.clone()
            } else {
                "unknown".to_string()
            };

            let meta_var = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: meta_var.clone(),
            });
            builder.emit(Instruction::MetaFunction {
                function_name,
                dest: meta_var.clone(),
            });
            metadata_vars.push(meta_var);
        }

        let choice_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: choice_var.clone(),
        });
        builder.emit(Instruction::LlmSelect {
            metadata_vars: metadata_vars.clone(),
            dest: choice_var.clone(),
        });

        for meta_var in &metadata_vars {
            builder.emit(Instruction::Drop {
                name: meta_var.clone(),
            });
        }

        builder.emit_switch(choice_var.clone(), clause_labels.clone());
        builder.emit(Instruction::Drop { name: choice_var });

        let end_label = format!("select_end_{}", builder.next_temp());

        for (i, clause) in select.clauses.iter().enumerate() {
            builder.emit_label(&clause_labels[i]);

            builder.emit(Instruction::CtxChild {
                is_scope_boundary: false,
            });

            let temp_result = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_result.clone(),
            });
            self.compile_expression(builder, &clause.expression_to_run, &temp_result)?;

            builder.emit(Instruction::Decl {
                name: clause.result_variable.clone(),
            });
            builder.emit(Instruction::Mov {
                dest: clause.result_variable.clone(),
                src: temp_result,
            });

            self.compile_expression(builder, &clause.expression_next, dest_var)?;

            builder.emit(Instruction::CtxRestore);
            builder.emit_br(&end_label);
        }

        builder.emit_label(&end_label);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_if_else_expression(
        &self,
        builder: &mut InstructionBuilder,
        condition: &typed_ast::Expression,
        then_expr: &typed_ast::Expression,
        else_expr: &typed_ast::Expression,
        dest_var: &str,
    ) -> Result<(), String> {
        let cond_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: cond_var.clone(),
        });
        self.compile_expression(builder, condition, &cond_var)?;

        let else_label = format!("ifelse_else_{}", builder.next_temp());
        let end_label = format!("ifelse_end_{}", builder.next_temp());

        builder.emit_brfalse(cond_var, &else_label);

        self.compile_expression(builder, then_expr, dest_var)?;
        builder.emit_br(&end_label);

        builder.emit_label(&else_label);
        self.compile_expression(builder, else_expr, dest_var)?;

        builder.emit_label(&end_label);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_struct_literal(
        &self,
        builder: &mut InstructionBuilder,
        struct_name: &str,
        fields: &[(String, typed_ast::Expression)],
        dest_var: &str,
    ) -> Result<(), String> {
        let mut field_vars = Vec::new();
        for (field_name, field_expr) in fields {
            let temp_var = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_var.clone(),
            });
            self.compile_expression(builder, field_expr, &temp_var)?;
            field_vars.push((field_name.clone(), temp_var));
        }
        builder.emit(Instruction::StructNew {
            dest: dest_var.to_string(),
            struct_name: struct_name.to_string(),
            fields: field_vars,
        });
        Ok(())
    }

    fn compile_field_access(
        &self,
        builder: &mut InstructionBuilder,
        base: &typed_ast::Expression,
        field: &str,
        dest_var: &str,
    ) -> Result<(), String> {
        let base_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: base_var.clone(),
        });
        self.compile_expression(builder, base, &base_var)?;
        builder.emit(Instruction::StructGet {
            dest: dest_var.to_string(),
            src: base_var,
            field: field.to_string(),
        });
        Ok(())
    }

    fn convert_type(ast_type: &ast::Type) -> crate::types::Type {
        match ast_type {
            ast::Type::Unit => crate::types::Type::Unit,
            ast::Type::Boolean => crate::types::Type::Boolean,
            ast::Type::String => crate::types::Type::String,
            ast::Type::Int => crate::types::Type::Int,
            ast::Type::Struct(name) => crate::types::Type::Struct(name.clone()),
            ast::Type::List(inner) => crate::types::Type::List(Box::new(Self::convert_type(inner))),
            ast::Type::Option(inner) => {
                crate::types::Type::Option(Box::new(Self::convert_type(inner)))
            }
            ast::Type::Generic(name) => crate::types::Type::Struct(name.clone()),
        }
    }

    fn type_to_string(ast_type: &ast::Type) -> String {
        format!("{}", ast_type)
    }
}

impl Default for BytecodeCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeCompiler {
    pub fn compile_function(
        &self,
        typed_func: &typed_ast::Function,
    ) -> Result<Box<dyn ExecutableFunction>, String> {
        let compiled = self.compile_to_bytecode(typed_func)?;
        let bytecode_expr = BytecodeFunctionExpr::new(compiled);
        Ok(Box::new(bytecode_expr))
    }
}

impl fmt::Display for CompiledFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {}(", self.name)?;
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                writeln!(f, ",")?;
            }
            write!(f, "    {}: {}", param.name, param.param_type.name())?;
        }
        writeln!(f, "\n): {} {{", self.return_type.name())?;

        let mut label_positions: Vec<(usize, &str)> = self
            .labels
            .iter()
            .map(|(name, pos)| (*pos, name.as_str()))
            .collect();
        label_positions.sort_by_key(|(pos, _)| *pos);

        let mut label_iter = label_positions.iter().peekable();

        for (i, instr) in self.instructions.iter().enumerate() {
            while let Some((pos, name)) = label_iter.peek() {
                if *pos == i {
                    writeln!(f, "  {}:", name)?;
                    label_iter.next();
                } else {
                    break;
                }
            }
            writeln!(f, "    {:3}: {}", i, instr)?;
        }

        writeln!(f, "}}")
    }
}
