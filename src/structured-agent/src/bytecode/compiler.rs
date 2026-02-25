use super::{BytecodeFunctionExpr, Instruction, builder::InstructionBuilder};
use crate::ast::{self, Expression, Statement};
use crate::compiler::FunctionCompiler;
use crate::expressions::FunctionExpr;
use crate::types::{Expression as ExpressionTrait, Function, Parameter};
use std::fmt;

pub struct CompiledFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: crate::types::Type,
    pub instructions: Vec<Instruction>,
    pub labels: std::collections::HashMap<String, usize>,
}

pub struct BytecodeCompiler;

impl BytecodeCompiler {
    pub fn compile_to_bytecode(ast_func: &ast::Function) -> Result<CompiledFunction, String> {
        let mut builder = InstructionBuilder::new();

        let mut has_explicit_return = false;
        for stmt in &ast_func.body.statements {
            if matches!(stmt, Statement::Return(_)) {
                has_explicit_return = true;
            }
            Self::compile_statement(&mut builder, stmt)?;
        }

        if !has_explicit_return {
            let return_temp = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: return_temp.clone(),
            });
            if ast_func.return_type == ast::Type::Unit {
                builder.emit(Instruction::LdcUnit {
                    dest: return_temp.clone(),
                });
            } else {
                let return_type_str = Self::type_to_string(&ast_func.return_type);
                builder.emit(Instruction::LlmGenerate {
                    dest: return_temp.clone(),
                    return_type: return_type_str,
                });
            }
            builder.emit(Instruction::Ret { var: return_temp });
        }

        let (instructions, labels) = builder.build()?;

        Ok(CompiledFunction {
            name: ast_func.name.clone(),
            parameters: ast_func
                .parameters
                .iter()
                .map(|p| Parameter::new(p.name.clone(), Self::convert_type(&p.param_type)))
                .collect(),
            return_type: Self::convert_type(&ast_func.return_type),
            instructions,
            labels,
        })
    }

    fn compile_statement(builder: &mut InstructionBuilder, stmt: &Statement) -> Result<(), String> {
        match stmt {
            Statement::Injection(expr) => Self::compile_injection(builder, expr),
            Statement::Assignment {
                variable,
                expression,
                ..
            } => Self::compile_assignment(builder, variable, expression),
            Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => Self::compile_variable_assignment(builder, variable, expression),
            Statement::ExpressionStatement(expr) => {
                Self::compile_expression_statement(builder, expr)
            }
            Statement::If {
                condition,
                body,
                else_body,
                ..
            } => Self::compile_if_statement(builder, condition, body, else_body.as_deref()),
            Statement::While {
                condition, body, ..
            } => Self::compile_while_statement(builder, condition, body),
            Statement::Return(expr) => Self::compile_return_statement(builder, expr),
        }
    }

    fn compile_injection(
        builder: &mut InstructionBuilder,
        expr: &Expression,
    ) -> Result<(), String> {
        let dest_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: dest_var.clone(),
        });
        Self::compile_expression(builder, expr, &dest_var)?;
        builder.emit(Instruction::CtxEvent {
            var: dest_var.clone(),
        });
        builder.emit_drop(dest_var);
        Ok(())
    }

    fn compile_assignment(
        builder: &mut InstructionBuilder,
        variable: &str,
        expression: &Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        Self::compile_expression(builder, expression, &temp_var)?;
        builder.emit(Instruction::Decl {
            name: variable.to_string(),
        });
        builder.emit(Instruction::Mov {
            dest: variable.to_string(),
            src: temp_var.clone(),
        });
        builder.emit_drop(temp_var);
        Ok(())
    }

    fn compile_variable_assignment(
        builder: &mut InstructionBuilder,
        variable: &str,
        expression: &Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        Self::compile_expression(builder, expression, &temp_var)?;
        builder.emit(Instruction::Mov {
            dest: variable.to_string(),
            src: temp_var.clone(),
        });
        builder.emit_drop(temp_var);
        Ok(())
    }

    fn compile_expression_statement(
        builder: &mut InstructionBuilder,
        expr: &Expression,
    ) -> Result<(), String> {
        let temp_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: temp_var.clone(),
        });
        Self::compile_expression(builder, expr, &temp_var)?;
        builder.emit_drop(temp_var);
        Ok(())
    }

    fn compile_if_statement(
        builder: &mut InstructionBuilder,
        condition: &Expression,
        body: &[Statement],
        else_body: Option<&[Statement]>,
    ) -> Result<(), String> {
        let if_start = format!("if_start_{}", builder.next_temp());
        builder.emit_label(&if_start);

        let cond_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: cond_var.clone(),
        });
        Self::compile_expression(builder, condition, &cond_var)?;

        let else_label = format!("else_{}", builder.next_temp());
        let end_label = format!("end_{}", builder.next_temp());

        builder.emit_brfalse(cond_var, &else_label);

        builder.emit(Instruction::CtxChild {
            is_scope_boundary: false,
        });
        for stmt in body {
            Self::compile_statement(builder, stmt)?;
        }
        builder.emit(Instruction::CtxRestore);
        builder.emit_br(&end_label);

        builder.emit_label(&else_label);
        if let Some(else_stmts) = else_body {
            builder.emit(Instruction::CtxChild {
                is_scope_boundary: false,
            });
            for stmt in else_stmts {
                Self::compile_statement(builder, stmt)?;
            }
            builder.emit(Instruction::CtxRestore);
        }

        builder.emit_label(&end_label);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_while_statement(
        builder: &mut InstructionBuilder,
        condition: &Expression,
        body: &[Statement],
    ) -> Result<(), String> {
        let loop_start = format!("loop_start_{}", builder.next_temp());
        let loop_end = format!("loop_end_{}", builder.next_temp());

        builder.emit_label(&loop_start);

        let cond_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: cond_var.clone(),
        });
        Self::compile_expression(builder, condition, &cond_var)?;
        builder.emit_brfalse(cond_var, &loop_end);

        builder.emit(Instruction::CtxChild {
            is_scope_boundary: false,
        });
        for stmt in body {
            Self::compile_statement(builder, stmt)?;
        }
        builder.emit(Instruction::CtxRestore);
        builder.emit_br(&loop_start);

        builder.emit_label(&loop_end);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_return_statement(
        builder: &mut InstructionBuilder,
        expr: &Expression,
    ) -> Result<(), String> {
        let result_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: result_var.clone(),
        });
        Self::compile_expression(builder, expr, &result_var)?;
        builder.emit(Instruction::Ret { var: result_var });
        Ok(())
    }

    fn compile_expression(
        builder: &mut InstructionBuilder,
        expr: &Expression,
        dest_var: &str,
    ) -> Result<(), String> {
        match expr {
            Expression::Call {
                function,
                arguments,
                ..
            } => Self::compile_call_expression(builder, function, arguments, dest_var),
            Expression::Variable { name, .. } => {
                Self::compile_variable_expression(builder, name, dest_var)
            }
            Expression::StringLiteral { value, .. } => {
                Self::compile_string_literal(builder, value, dest_var)
            }
            Expression::BooleanLiteral { value, .. } => {
                Self::compile_boolean_literal(builder, *value, dest_var)
            }
            Expression::UnitLiteral { .. } => Self::compile_unit_literal(builder, dest_var),
            Expression::ListLiteral { elements, .. } => {
                Self::compile_list_literal(builder, elements, dest_var)
            }
            Expression::Placeholder { .. } => Self::compile_placeholder(builder, dest_var),
            Expression::Select(select_expr) => {
                Self::compile_select_expression(builder, select_expr, dest_var)
            }
            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                Self::compile_if_else_expression(builder, condition, then_expr, else_expr, dest_var)
            }
        }
    }

    fn compile_call_expression(
        builder: &mut InstructionBuilder,
        function: &str,
        arguments: &[Expression],
        dest_var: &str,
    ) -> Result<(), String> {
        let mut params = Vec::new();

        for arg_expr in arguments {
            let temp_var = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_var.clone(),
            });
            Self::compile_expression(builder, arg_expr, &temp_var)?;
            params.push(temp_var);
        }

        builder.emit(Instruction::Call {
            function_name: function.to_string(),
            params,
            dest: dest_var.to_string(),
        });
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
        builder: &mut InstructionBuilder,
        elements: &[Expression],
        dest_var: &str,
    ) -> Result<(), String> {
        let element_type = "Unknown".to_string();
        let mut temp_vars = Vec::new();

        for elem in elements {
            let temp_var = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_var.clone(),
            });
            Self::compile_expression(builder, elem, &temp_var)?;
            temp_vars.push(temp_var);
        }

        builder.emit(Instruction::ListNew {
            dest: dest_var.to_string(),
            element_type,
        });

        for temp_var in temp_vars {
            builder.emit(Instruction::ListAdd {
                dest: dest_var.to_string(),
                src: temp_var,
            });
        }

        builder.emit(Instruction::ListFinish {
            dest: dest_var.to_string(),
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
        builder: &mut InstructionBuilder,
        select_expr: &ast::SelectExpression,
        dest_var: &str,
    ) -> Result<(), String> {
        let select_start = format!("select_start_{}", builder.next_temp());
        builder.emit_label(&select_start);

        let clause_count = select_expr.clauses.len();
        builder.emit(Instruction::SelectBegin { clause_count });

        builder.emit(Instruction::Decl {
            name: dest_var.to_string(),
        });

        let mut clause_labels = Vec::new();
        for i in 0..clause_count {
            let label = format!("clause_{}_{}", i, builder.next_temp());
            clause_labels.push(label.clone());

            let function_name = if let Expression::Call { function, .. } =
                &select_expr.clauses[i].expression_to_run
            {
                function.clone()
            } else {
                "unknown".to_string()
            };

            builder.emit(Instruction::SelectClause {
                function_name,
                offset: 0,
            });
        }

        let choice_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: choice_var.clone(),
        });
        builder.emit(Instruction::LlmSelect {
            dest: choice_var.clone(),
        });

        builder.emit_switch(choice_var, clause_labels.clone());

        let end_label = format!("select_end_{}", builder.next_temp());

        for (i, clause) in select_expr.clauses.iter().enumerate() {
            builder.emit_label(&clause_labels[i]);

            builder.emit(Instruction::CtxChild {
                is_scope_boundary: false,
            });

            let temp_result = builder.next_temp();
            builder.emit(Instruction::Decl {
                name: temp_result.clone(),
            });
            Self::compile_expression(builder, &clause.expression_to_run, &temp_result)?;

            builder.emit(Instruction::Decl {
                name: clause.result_variable.clone(),
            });
            builder.emit(Instruction::Mov {
                dest: clause.result_variable.clone(),
                src: temp_result,
            });

            Self::compile_expression(builder, &clause.expression_next, dest_var)?;

            builder.emit(Instruction::CtxRestore);

            builder.emit_br(&end_label);
        }

        builder.emit_label(&end_label);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn compile_if_else_expression(
        builder: &mut InstructionBuilder,
        condition: &Expression,
        then_expr: &Expression,
        else_expr: &Expression,
        dest_var: &str,
    ) -> Result<(), String> {
        let cond_var = builder.next_temp();
        builder.emit(Instruction::Decl {
            name: cond_var.clone(),
        });
        Self::compile_expression(builder, condition, &cond_var)?;

        let else_label = format!("ifelse_else_{}", builder.next_temp());
        let end_label = format!("ifelse_end_{}", builder.next_temp());

        builder.emit_brfalse(cond_var, &else_label);

        Self::compile_expression(builder, then_expr, dest_var)?;
        builder.emit_br(&end_label);

        builder.emit_label(&else_label);
        Self::compile_expression(builder, else_expr, dest_var)?;

        builder.emit_label(&end_label);
        builder.emit(Instruction::Nop);
        Ok(())
    }

    fn convert_type(ast_type: &ast::Type) -> crate::types::Type {
        match ast_type {
            ast::Type::Unit => crate::types::Type::Unit,
            ast::Type::Boolean => crate::types::Type::Boolean,
            ast::Type::String => crate::types::Type::String,
            ast::Type::List(inner) => crate::types::Type::List(Box::new(Self::convert_type(inner))),
            ast::Type::Option(inner) => {
                crate::types::Type::Option(Box::new(Self::convert_type(inner)))
            }
        }
    }

    fn generate_param_names(count: usize) -> Vec<String> {
        (0..count).map(|i| format!("arg{}", i)).collect()
    }

    fn type_to_string(ast_type: &ast::Type) -> String {
        match ast_type {
            ast::Type::Unit => "Unit".to_string(),
            ast::Type::Boolean => "Boolean".to_string(),
            ast::Type::String => "String".to_string(),
            ast::Type::List(inner) => format!("List<{}>", Self::type_to_string(inner)),
            ast::Type::Option(inner) => format!("Option<{}>", Self::type_to_string(inner)),
        }
    }
}

impl FunctionCompiler for BytecodeCompiler {
    fn compile_function(ast_func: &ast::Function) -> Result<FunctionExpr, String> {
        let compiled = Self::compile_to_bytecode(ast_func)?;
        let bytecode_expr = BytecodeFunctionExpr::new(compiled);

        Ok(FunctionExpr {
            name: Function::name(&bytecode_expr).to_string(),
            parameters: Function::parameters(&bytecode_expr).to_vec(),
            return_type: bytecode_expr.return_type(),
            body: vec![Box::new(bytecode_expr) as Box<dyn ExpressionTrait>],
            documentation: ast_func.documentation.clone(),
        })
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
