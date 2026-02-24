use super::{Instruction, builder::InstructionBuilder};
use crate::ast::{self, Expression, Statement};
use crate::types::Parameter;
use std::fmt;

pub struct CompiledFunction {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: crate::types::Type,
    pub instructions: Vec<Instruction>,
}

pub struct BytecodeCompiler;

impl BytecodeCompiler {
    pub fn compile_function(ast_func: &ast::Function) -> Result<CompiledFunction, String> {
        let mut builder = InstructionBuilder::new();

        for stmt in &ast_func.body.statements {
            Self::compile_statement(&mut builder, stmt)?;
        }

        let instructions = builder.build()?;

        Ok(CompiledFunction {
            name: ast_func.name.clone(),
            parameters: ast_func
                .parameters
                .iter()
                .map(|p| Parameter::new(p.name.clone(), Self::convert_type(&p.param_type)))
                .collect(),
            return_type: Self::convert_type(&ast_func.return_type),
            instructions,
        })
    }

    fn compile_statement(builder: &mut InstructionBuilder, stmt: &Statement) -> Result<(), String> {
        match stmt {
            Statement::Injection(expr) => {
                let dest_var = builder.next_temp();
                Self::compile_expression(builder, expr, &dest_var)?;
                builder.emit(Instruction::CtxEvent { var: dest_var });
            }

            Statement::Assignment {
                variable,
                expression,
                ..
            } => {
                let temp_var = builder.next_temp();
                Self::compile_expression(builder, expression, &temp_var)?;
                builder.emit(Instruction::Decl {
                    name: variable.clone(),
                });
                builder.emit(Instruction::Mov {
                    dest: variable.clone(),
                    src: temp_var,
                });
            }

            Statement::VariableAssignment {
                variable,
                expression,
                ..
            } => {
                let temp_var = builder.next_temp();
                Self::compile_expression(builder, expression, &temp_var)?;
                builder.emit(Instruction::Mov {
                    dest: variable.clone(),
                    src: temp_var,
                });
            }

            Statement::ExpressionStatement(expr) => {
                let temp_var = builder.next_temp();
                Self::compile_expression(builder, expr, &temp_var)?;
            }

            Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                let cond_var = builder.next_temp();
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
            }

            Statement::While {
                condition, body, ..
            } => {
                let loop_start = format!("loop_start_{}", builder.next_temp());
                let loop_end = format!("loop_end_{}", builder.next_temp());

                builder.emit_label(&loop_start);

                let cond_var = builder.next_temp();
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
            }

            Statement::Return(expr) => {
                let result_var = builder.next_temp();
                Self::compile_expression(builder, expr, &result_var)?;
                builder.emit(Instruction::Ret { var: result_var });
            }
        }

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
            } => {
                builder.emit(Instruction::CallBegin {
                    function_name: function.clone(),
                });

                for (arg_expr, param_name) in arguments
                    .iter()
                    .zip(Self::generate_param_names(arguments.len()))
                {
                    let temp_var = builder.next_temp();
                    Self::compile_expression(builder, arg_expr, &temp_var)?;
                    builder.emit(Instruction::CallArg {
                        param_name,
                        var: temp_var,
                    });
                }

                builder.emit(Instruction::CallInvoke {
                    dest: dest_var.to_string(),
                });
            }

            Expression::Variable { name, .. } => {
                builder.emit(Instruction::Mov {
                    dest: dest_var.to_string(),
                    src: name.clone(),
                });
            }

            Expression::StringLiteral { value, .. } => {
                builder.emit(Instruction::LdcStr {
                    dest: dest_var.to_string(),
                    value: value.clone(),
                });
            }

            Expression::BooleanLiteral { value, .. } => {
                builder.emit(Instruction::LdcBool {
                    dest: dest_var.to_string(),
                    value: *value,
                });
            }

            Expression::UnitLiteral { .. } => {
                builder.emit(Instruction::LdcUnit {
                    dest: dest_var.to_string(),
                });
            }

            Expression::ListLiteral { elements, .. } => {
                let element_type = "Unknown".to_string();
                let mut temp_vars = Vec::new();

                for elem in elements {
                    let temp_var = builder.next_temp();
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
            }

            Expression::Placeholder { .. } => {
                builder.emit(Instruction::LlmPlaceholder {
                    dest: dest_var.to_string(),
                    param_name: "placeholder".to_string(),
                    param_type: "Unknown".to_string(),
                });
            }

            Expression::Select(select_expr) => {
                let clause_count = select_expr.clauses.len();
                builder.emit(Instruction::SelectBegin { clause_count });

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
            }

            Expression::IfElse {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let cond_var = builder.next_temp();
                Self::compile_expression(builder, condition, &cond_var)?;

                let else_label = format!("ifelse_else_{}", builder.next_temp());
                let end_label = format!("ifelse_end_{}", builder.next_temp());

                builder.emit_brfalse(cond_var, &else_label);

                Self::compile_expression(builder, then_expr, dest_var)?;
                builder.emit_br(&end_label);

                builder.emit_label(&else_label);
                Self::compile_expression(builder, else_expr, dest_var)?;

                builder.emit_label(&end_label);
            }
        }

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

        for (i, instr) in self.instructions.iter().enumerate() {
            writeln!(f, "    {:3}: {}", i, instr)?;
        }

        writeln!(f, "}}")
    }
}
