#[cfg(test)]
mod instruction_display_tests {
    use crate::bytecode::Instruction;
    use structured_agent_runtime::{FunctionName, FunctionNameKind, ModuleName};

    #[test]
    fn test_ldc_str_display() {
        let instr = Instruction::LdcStr {
            dest: "x".to_string(),
            value: "hello".to_string(),
        };
        assert_eq!(format!("{}", instr), "ldc.str x, \"hello\"");
    }

    #[test]
    fn test_call_bytecode_display() {
        let instr = Instruction::CallBytecode {
            function_name: FunctionName {
                name: "foo".to_string(),
                module: ModuleName::from_str("mymod"),
                kind: FunctionNameKind::Function,
            },
            params: vec!["x".to_string(), "y".to_string()],
            dest: "result".to_string(),
        };
        assert_eq!(
            format!("{}", instr),
            "call.bytecode mymod::foo, [x, y], result"
        );
    }

    #[test]
    fn test_call_external_display() {
        let instr = Instruction::CallExternal {
            function_name: FunctionName {
                name: "foo".to_string(),
                module: ModuleName::from_str("mymod"),
                kind: FunctionNameKind::Function,
            },
            params: vec!["x".to_string(), "y".to_string()],
            dest: "result".to_string(),
        };
        assert_eq!(
            format!("{}", instr),
            "call.external mymod::foo, [x, y], result"
        );
    }

    #[test]
    fn test_br_display() {
        let instr = Instruction::Br { offset: 5 };
        assert_eq!(format!("{}", instr), "br 5");
    }

    #[test]
    fn test_ctx_child_display() {
        let instr = Instruction::CtxChild {
            is_scope_boundary: true,
        };
        assert_eq!(format!("{}", instr), "ctx.child true");
    }

    #[test]
    fn test_struct_new_display() {
        let instr = Instruction::StructNew {
            dest: "p".to_string(),
            struct_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), "$tmp0".to_string()),
                ("y".to_string(), "$tmp1".to_string()),
            ],
        };
        assert_eq!(
            format!("{}", instr),
            "struct.new p, Point, {x: $tmp0, y: $tmp1}"
        );
    }

    #[test]
    fn test_struct_get_display() {
        let instr = Instruction::StructGet {
            dest: "$tmp0".to_string(),
            src: "p".to_string(),
            field: "x".to_string(),
        };
        assert_eq!(format!("{}", instr), "struct.get $tmp0, p, x");
    }
}

#[cfg(test)]
mod compilation_tests {
    use crate::bytecode::BytecodeCompiler;
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;
    use crate::typecheck::TypeChecker;
    use crate::typecheck::checker::TypedCheckerAstRef;
    use crate::typed_ast;
    use std::collections::HashMap;

    fn parse_code(code: &str) -> crate::ast::Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new();
        parser.parse(&unit, file_id, manager.reporter()).unwrap()
    }

    fn parse_and_typecheck(code: &str) -> typed_ast::Module {
        let module = parse_code(code);
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parsed = crate::ast::ParsedModule {
            name: "".to_string(),
            module,
            is_entry: false,
            file_id,
        };
        let (_, typed_metadata) = TypeChecker::new()
            .check_modules(&[parsed], &HashMap::new())
            .unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module.to_string() != "" {
                    return None;
                }
                if let TypedCheckerAstRef::Function(func, _) = &f.ast_ref {
                    Some(typed_ast::Definition::Function((**func).clone()))
                } else {
                    None
                }
            })
            .collect();
        typed_ast::Module {
            definitions,
            span: crate::types::Span::dummy(),
            file_id,
        }
    }

    #[test]
    fn test_display_choose_message_bytecode() {
        let code = r#"
fn choose_message(ready: Boolean): String {
    return if ready { "System ready" } else { "System not ready" }
}

fn main(): String {
    let message = choose_message(true)
    message!
}
"#;
        let module = parse_and_typecheck(code);

        for def in &module.definitions {
            if let typed_ast::Definition::Function(func) = def {
                let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();
                println!("\n{}", compiled);
            }
        }
    }

    #[test]
    fn test_display_select_bytecode() {
        let code = r#"
fn add(a: String, b: String): String {
    "Adding numbers"
}

fn subtract(a: String, b: String): String {
    "Subtracting numbers"
}

fn calculator(x: String, y: String): String {
    let result = select {
        add(x, y) as sum => sum,
        subtract(x, y) as diff => diff
    }
    result
}

fn main(): String {
    let result = calculator("5", "3")
    result!
}
"#;
        let module = parse_and_typecheck(code);

        for def in &module.definitions {
            if let typed_ast::Definition::Function(func) = def {
                let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();
                println!("\n{}", compiled);
            }
        }
    }

    fn get_function<'a>(module: &'a typed_ast::Module, name: &str) -> &'a typed_ast::Function {
        for def in &module.definitions {
            if let typed_ast::Definition::Function(f) = def {
                if f.name == name {
                    return f;
                }
            }
        }
        panic!("Function '{}' not found in module", name);
    }

    fn compile_and_check(code: &str, expected: &str) {
        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();
        assert_eq!(format!("{}", compiled), expected);
    }

    fn compile_and_check_named(code: &str, function_name: &str, expected: &str) {
        let module = parse_and_typecheck(code);
        let func = get_function(&module, function_name);
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();
        assert_eq!(format!("{}", compiled), expected);
    }

    #[test]
    fn test_compile_string_literal() {
        let code = r#"
            fn test(): String {
                return "hello"
            }
        "#;

        let expected = r#"fn test(

): String {
      0: decl $tmp0
      1: ldc.str $tmp0, "hello"
      2: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_boolean_literal() {
        let code = r#"
            fn test(): Boolean {
                return true
            }
        "#;

        let expected = r#"fn test(

): Boolean {
      0: decl $tmp0
      1: ldc.bool $tmp0, true
      2: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_assignment() {
        let code = r#"
            fn test(): () {
                let x = "test"
            }
        "#;

        let expected = r#"fn test(

): () {
      0: decl $tmp0
      1: ldc.str $tmp0, "test"
      2: decl x
      3: mov x, $tmp0
      4: drop $tmp0
      5: decl $tmp1
      6: ldc.unit $tmp1
      7: ret $tmp1
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_injection() {
        let code = r#"
            fn test(): () {
                "event"!
            }
        "#;

        let expected = r#"fn test(

): () {
      0: decl $tmp0
      1: ldc.str $tmp0, "event"
      2: ctx.event $tmp0
      3: drop $tmp0
      4: decl $tmp1
      5: ldc.unit $tmp1
      6: ret $tmp1
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_function_call() {
        let code = r#"
            extern fn foo(a: String, b: Boolean): String
            fn test(): String {
                return foo("arg1", true)
            }
        "#;

        let expected = r#"fn test(

): String {
      0: decl $tmp0
      1: decl $tmp1
      2: ldc.str $tmp1, "arg1"
      3: decl $tmp2
      4: ldc.bool $tmp2, true
      5: call.external foo, [$tmp1, $tmp2], $tmp0
      6: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_if_statement() {
        let code = r#"
            fn test(): () {
                if true {
                    "then"!
                } else {
                    "else"!
                }
            }
        "#;

        let expected = r#"fn test(

): () {
  if_start_$tmp0:
      0: decl $tmp1
      1: ldc.bool $tmp1, true
      2: brfalse $tmp1, 10
      3: ctx.child false
      4: decl $tmp4
      5: ldc.str $tmp4, "then"
      6: ctx.event $tmp4
      7: drop $tmp4
      8: ctx.restore
      9: br 16
  else_$tmp2:
     10: ctx.child false
     11: decl $tmp5
     12: ldc.str $tmp5, "else"
     13: ctx.event $tmp5
     14: drop $tmp5
     15: ctx.restore
  end_$tmp3:
     16: nop
     17: decl $tmp6
     18: ldc.unit $tmp6
     19: ret $tmp6
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_while_loop() {
        let code = r#"
            fn test(): () {
                while true {
                    "loop"!
                }
            }
        "#;

        let expected = r#"fn test(

): () {
  loop_start_$tmp0:
      0: decl $tmp2
      1: ldc.bool $tmp2, true
      2: brfalse $tmp2, 10
      3: ctx.child false
      4: decl $tmp3
      5: ldc.str $tmp3, "loop"
      6: ctx.event $tmp3
      7: drop $tmp3
      8: ctx.restore
      9: br 0
  loop_end_$tmp1:
     10: nop
     11: decl $tmp4
     12: ldc.unit $tmp4
     13: ret $tmp4
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_list_literal() {
        let code = r#"
            fn test(): List<String> {
                return ["a", "b"]
            }
        "#;

        let expected = r#"fn test(

): List<String> {
      0: decl $tmp0
      1: decl $tmp1
      2: ldc.str $tmp1, "a"
      3: decl $tmp2
      4: ldc.str $tmp2, "b"
      5: list.create $tmp0, [$tmp1, $tmp2]
      6: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_example_compilation_output() {
        let code = r#"
## Greet a user
fn greet(name: String): () {
    let message = "Hello"
    message!
}
        "#;

        let expected = r#"fn greet(
    name: String
): () {
      0: decl $tmp0
      1: ldc.str $tmp0, "Hello"
      2: decl message
      3: mov message, $tmp0
      4: drop $tmp0
      5: decl $tmp1
      6: mov $tmp1, message
      7: ctx.event $tmp1
      8: drop $tmp1
      9: decl $tmp2
     10: ldc.unit $tmp2
     11: ret $tmp2
}
"#;
        compile_and_check_named(code, "greet", expected);
    }

    #[test]
    fn test_pretty_print_function() {
        let code = r#"
            extern fn process(x: String): String
            fn calculate(x: String, y: Boolean): String {
                let result = process(x)
                return result
            }
        "#;

        let expected = r#"fn calculate(
    x: String,
    y: Boolean
): String {
      0: decl $tmp0
      1: decl $tmp1
      2: mov $tmp1, x
      3: call.external process, [$tmp1], $tmp0
      4: decl result
      5: mov result, $tmp0
      6: drop $tmp0
      7: decl $tmp2
      8: mov $tmp2, result
      9: ret $tmp2
}
"#;
        compile_and_check_named(code, "calculate", expected);
    }

    #[test]
    fn test_complex_function_pretty_print() {
        let code = r#"
            extern fn transform(items: List<String>): String
            fn process_items(items: List<String>, filter: Boolean): String {
                let result = "initial"
                if filter {
                    result = transform(items)
                    result!
                } else {
                    "skipped"!
                }
                return result
            }
        "#;

        let expected = r#"fn process_items(
    items: List<String>,
    filter: Boolean
): String {
      0: decl $tmp0
      1: ldc.str $tmp0, "initial"
      2: decl result
      3: mov result, $tmp0
      4: drop $tmp0
  if_start_$tmp1:
      5: decl $tmp2
      6: mov $tmp2, filter
      7: brfalse $tmp2, 21
      8: ctx.child false
      9: decl $tmp5
     10: decl $tmp6
     11: mov $tmp6, items
     12: call.external transform, [$tmp6], $tmp5
     13: mov result, $tmp5
     14: drop $tmp5
     15: decl $tmp7
     16: mov $tmp7, result
     17: ctx.event $tmp7
     18: drop $tmp7
     19: ctx.restore
     20: br 27
  else_$tmp3:
     21: ctx.child false
     22: decl $tmp8
     23: ldc.str $tmp8, "skipped"
     24: ctx.event $tmp8
     25: drop $tmp8
     26: ctx.restore
  end_$tmp4:
     27: nop
     28: decl $tmp9
     29: mov $tmp9, result
     30: ret $tmp9
}
"#;
        compile_and_check_named(code, "process_items", expected);
    }

    #[test]
    fn test_select_expression() {
        let code = r#"
            extern fn analyze(code: String): String
            extern fn summarize(text: String): String
            fn test(): String {
                return select {
                    analyze("code") as result => result,
                    summarize("text") as summary => summary
                }
            }
        "#;

        let expected = r#"fn test(

): String {
      0: decl $tmp0
  select_start_$tmp1:
      1: decl $tmp0
      2: decl $tmp3
      3: meta.function analyze, $tmp3
      4: decl $tmp5
      5: meta.function summarize, $tmp5
      6: decl $tmp6
      7: llm.select [$tmp3, $tmp5], $tmp6
      8: drop $tmp3
      9: drop $tmp5
     10: switch $tmp6, [12, 22]
     11: drop $tmp6
  clause_0_$tmp2:
     12: ctx.child false
     13: decl $tmp8
     14: decl $tmp9
     15: ldc.str $tmp9, "code"
     16: call.external analyze, [$tmp9], $tmp8
     17: decl result
     18: mov result, $tmp8
     19: mov $tmp0, result
     20: ctx.restore
     21: br 32
  clause_1_$tmp4:
     22: ctx.child false
     23: decl $tmp10
     24: decl $tmp11
     25: ldc.str $tmp11, "text"
     26: call.external summarize, [$tmp11], $tmp10
     27: decl summary
     28: mov summary, $tmp10
     29: mov $tmp0, summary
     30: ctx.restore
     31: br 32
  select_end_$tmp7:
     32: nop
     33: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_if_else_expression() {
        let code = r#"
            fn test(x: Boolean): String {
                return if x { "yes" } else { "no" }
            }
        "#;

        let expected = r#"fn test(
    x: Boolean
): String {
      0: decl $tmp0
      1: decl $tmp1
      2: mov $tmp1, x
      3: brfalse $tmp1, 6
      4: ldc.str $tmp0, "yes"
      5: br 7
  ifelse_else_$tmp2:
      6: ldc.str $tmp0, "no"
  ifelse_end_$tmp3:
      7: nop
      8: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_variable_assignment() {
        let code = r#"
            fn test(): String {
                let x = "initial"
                x = "updated"
                return x
            }
        "#;

        let expected = r#"fn test(

): String {
      0: decl $tmp0
      1: ldc.str $tmp0, "initial"
      2: decl x
      3: mov x, $tmp0
      4: drop $tmp0
      5: decl $tmp1
      6: ldc.str $tmp1, "updated"
      7: mov x, $tmp1
      8: drop $tmp1
      9: decl $tmp2
     10: mov $tmp2, x
     11: ret $tmp2
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_struct_literal() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn test(): Point {
    return Point { x: 1, y: 2 }
}
"#;
        let expected = r#"fn test(

): Point {
      0: decl $tmp0
      1: decl $tmp1
      2: ldc.int $tmp1, 1
      3: decl $tmp2
      4: ldc.int $tmp2, 2
      5: struct.new $tmp0, Point, {x: $tmp1, y: $tmp2}
      6: ret $tmp0
}
"#;
        compile_and_check_named(code, "test", expected);
    }

    #[test]
    fn test_compile_field_access() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn test(p: Point): Int {
    return p.x
}
"#;
        let expected = r#"fn test(
    p: Point
): Int {
      0: decl $tmp0
      1: decl $tmp1
      2: mov $tmp1, p
      3: struct.get $tmp0, $tmp1, x
      4: ret $tmp0
}
"#;
        compile_and_check_named(code, "test", expected);
    }

    #[test]
    fn test_compile_chained_field_access() {
        let code = r#"
struct Address {
    city: String,
}
struct Person {
    address: Address,
}
fn test(p: Person): String {
    return p.address.city
}
"#;
        let expected = r#"fn test(
    p: Person
): String {
      0: decl $tmp0
      1: decl $tmp1
      2: decl $tmp2
      3: mov $tmp2, p
      4: struct.get $tmp1, $tmp2, address
      5: struct.get $tmp0, $tmp1, city
      6: ret $tmp0
}
"#;
        compile_and_check_named(code, "test", expected);
    }

    #[test]
    fn test_compile_field_access_on_call() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn make_point(): Point {
    return Point { x: 5, y: 10 }
}
fn test(): Int {
    return make_point().x
}
"#;
        let expected = r#"fn test(

): Int {
      0: decl $tmp0
      1: decl $tmp1
      2: call.bytecode make_point, [], $tmp1
      3: struct.get $tmp0, $tmp1, x
      4: ret $tmp0
}
"#;
        compile_and_check_named(code, "test", expected);
    }

    #[test]
    fn test_compile_placeholder() {
        let code = r#"
            extern fn foo(x: String): String
            fn test(): String {
                return foo(_)
            }
        "#;

        let expected = r#"fn test(

): String {
      0: decl $tmp0
      1: decl $tmp1
      2: llm.placeholder $tmp1, placeholder, Unknown
      3: call.external foo, [$tmp1], $tmp0
      4: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_unit_literal() {
        let code = r#"
            fn test(): () {
                return ()
            }
        "#;

        let expected = r#"fn test(

): () {
      0: decl $tmp0
      1: ldc.unit $tmp0
      2: ret $tmp0
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_unit_literal_in_variable() {
        let code = r#"
            fn test(): () {
                let x = ()
                return x
            }
        "#;

        let expected = r#"fn test(

): () {
      0: decl $tmp0
      1: ldc.unit $tmp0
      2: decl x
      3: mov x, $tmp0
      4: drop $tmp0
      5: decl $tmp1
      6: mov $tmp1, x
      7: ret $tmp1
}
"#;
        compile_and_check(code, expected);
    }
}

#[cfg(test)]
mod vm_execution_tests {
    use crate::bytecode::{BytecodeCompiler, BytecodeFunctionExpr, VM};
    use crate::cli::config::ProgramSource;
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;
    use crate::runtime::{Context, ExpressionValue, Runtime};
    use crate::typecheck::TypeChecker;
    use crate::typecheck::checker::TypedCheckerAstRef;
    use crate::typed_ast;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_runtime::{FunctionName, FunctionNameKind, ModuleName};

    fn parse_code(code: &str) -> crate::ast::Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new();
        parser.parse(&unit, file_id, manager.reporter()).unwrap()
    }

    fn parse_and_typecheck(code: &str) -> typed_ast::Module {
        let module = parse_code(code);
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parsed = crate::ast::ParsedModule {
            name: "".to_string(),
            module,
            is_entry: false,
            file_id,
        };
        let (_, typed_metadata) = TypeChecker::new()
            .check_modules(&[parsed], &HashMap::new())
            .unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module.to_string() != "" {
                    return None;
                }
                if let TypedCheckerAstRef::Function(func, _) = &f.ast_ref {
                    Some(typed_ast::Definition::Function((**func).clone()))
                } else {
                    None
                }
            })
            .collect();
        typed_ast::Module {
            definitions,
            span: crate::types::Span::dummy(),
            file_id,
        }
    }

    fn get_function<'a>(module: &'a typed_ast::Module, name: &str) -> &'a typed_ast::Function {
        for def in &module.definitions {
            if let typed_ast::Definition::Function(f) = def {
                if f.name == name {
                    return f;
                }
            }
        }
        panic!("Function '{}' not found in module", name);
    }

    fn get_ast_function<'a>(
        module: &'a crate::ast::Module,
        name: &str,
    ) -> &'a crate::ast::Function {
        for def in &module.definitions {
            if let crate::ast::Definition::Function(f) = def {
                if f.name == name {
                    return f;
                }
            }
        }
        panic!("Function '{}' not found in module", name);
    }

    fn ast_expr_to_typed(expr: &crate::ast::Expression) -> typed_ast::Expression {
        use crate::ast::Expression as AE;
        use crate::ast::Type;
        let dummy = crate::types::Span::dummy();
        match expr {
            AE::StringLiteral { value, span } => typed_ast::Expression::StringLiteral {
                value: value.clone(),
                ty: Type::String,
                span: *span,
            },
            AE::BooleanLiteral { value, span } => typed_ast::Expression::BooleanLiteral {
                value: *value,
                ty: Type::Boolean,
                span: *span,
            },
            AE::IntLiteral { value, span } => typed_ast::Expression::IntLiteral {
                value: *value,
                ty: Type::Int,
                span: *span,
            },
            AE::Variable { name, span } => typed_ast::Expression::Variable {
                name: name.clone(),
                ty: Type::Unit,
                span: *span,
            },
            AE::UnitLiteral { span } => typed_ast::Expression::UnitLiteral {
                ty: Type::Unit,
                span: *span,
            },
            AE::Placeholder { span } => typed_ast::Expression::Placeholder {
                ty: Type::Unit,
                span: *span,
            },
            AE::Call {
                function,
                arguments,
                span,
            } => typed_ast::Expression::Call {
                function: function.clone(),
                resolved: FunctionName {
                    name: function.to_string(),
                    module: ModuleName::from_str("test"),
                    kind: FunctionNameKind::Function,
                },
                kind: crate::typecheck::checker::FunctionKind::External,
                arguments: arguments.iter().map(ast_expr_to_typed).collect(),
                ty: Type::Unit,
                span: *span,
            },
            AE::ListLiteral { elements, span } => typed_ast::Expression::ListLiteral {
                elements: elements.iter().map(ast_expr_to_typed).collect(),
                ty: Type::Unit,
                span: *span,
            },
            AE::StructLiteral {
                struct_name,
                fields,
                span,
            } => typed_ast::Expression::StructLiteral {
                struct_name: struct_name.clone(),
                fields: fields
                    .iter()
                    .map(|(n, e)| (n.clone(), ast_expr_to_typed(e)))
                    .collect(),
                ty: Type::Unit,
                span: *span,
            },
            AE::FieldAccess { base, field, span } => typed_ast::Expression::FieldAccess {
                base: Box::new(ast_expr_to_typed(base)),
                field: field.clone(),
                ty: Type::Unit,
                span: *span,
            },
            AE::IfElse {
                condition,
                then_expr,
                else_expr,
                span,
            } => typed_ast::Expression::IfElse {
                condition: Box::new(ast_expr_to_typed(condition)),
                then_expr: Box::new(ast_expr_to_typed(then_expr)),
                else_expr: Box::new(ast_expr_to_typed(else_expr)),
                ty: Type::Unit,
                span: *span,
            },
            AE::Select(select) => typed_ast::Expression::Select(
                typed_ast::SelectExpression {
                    clauses: select
                        .clauses
                        .iter()
                        .map(|c| typed_ast::SelectClause {
                            expression_to_run: ast_expr_to_typed(&c.expression_to_run),
                            result_variable: c.result_variable.clone(),
                            expression_next: ast_expr_to_typed(&c.expression_next),
                            span: c.span,
                        })
                        .collect(),
                    span: select.span,
                },
                crate::ast::Type::Unit,
            ),
        }
    }

    fn ast_stmt_to_typed(stmt: &crate::ast::Statement) -> typed_ast::Statement {
        use crate::ast::Statement as AS;
        match stmt {
            AS::Injection(expr) => typed_ast::Statement::Injection(ast_expr_to_typed(expr)),
            AS::Assignment {
                variable,
                expression,
                span,
            } => typed_ast::Statement::Assignment {
                variable: variable.clone(),
                expression: ast_expr_to_typed(expression),
                span: *span,
            },
            AS::VariableAssignment {
                variable,
                expression,
                span,
            } => typed_ast::Statement::VariableAssignment {
                variable: variable.clone(),
                expression: ast_expr_to_typed(expression),
                span: *span,
            },
            AS::ExpressionStatement(expr) => {
                typed_ast::Statement::ExpressionStatement(ast_expr_to_typed(expr))
            }
            AS::If {
                condition,
                body,
                else_body,
                span,
            } => typed_ast::Statement::If {
                condition: ast_expr_to_typed(condition),
                body: body.iter().map(ast_stmt_to_typed).collect(),
                else_body: else_body
                    .as_ref()
                    .map(|stmts| stmts.iter().map(ast_stmt_to_typed).collect()),
                span: *span,
            },
            AS::While {
                condition,
                body,
                span,
            } => typed_ast::Statement::While {
                condition: ast_expr_to_typed(condition),
                body: body.iter().map(ast_stmt_to_typed).collect(),
                span: *span,
            },
            AS::Return(expr) => typed_ast::Statement::Return(ast_expr_to_typed(expr)),
        }
    }

    fn ast_func_to_typed(f: &crate::ast::Function) -> typed_ast::Function {
        typed_ast::Function {
            name: f.name.clone(),
            parameters: f.parameters.clone(),
            return_type: f.return_type.clone(),
            body: typed_ast::FunctionBody {
                statements: f.body.statements.iter().map(ast_stmt_to_typed).collect(),
                span: f.body.span,
            },
            documentation: f.documentation.clone(),
            is_pub: f.is_pub,
            span: f.span,
        }
    }

    #[tokio::test]
    async fn test_vm_string_literal() {
        let code = r#"
            fn test(): String {
                return "hello"
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.as_string().unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_vm_boolean_literal() {
        let code = r#"
            fn test(): Boolean {
                return true
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.as_boolean().unwrap(), true);
    }

    #[tokio::test]
    async fn test_vm_unit_literal() {
        let code = r#"
            fn test(): () {
                return ()
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.type_name(), "Unit");
    }

    #[tokio::test]
    async fn test_vm_assignment_and_variable() {
        let code = r#"
            fn test(): String {
                let x = "test"
                return x
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.as_string().unwrap(), "test");
    }

    #[tokio::test]
    async fn test_vm_variable_reassignment() {
        let code = r#"
            fn test(): String {
                let x = "initial"
                x = "updated"
                return x
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.as_string().unwrap(), "updated");
    }

    #[tokio::test]
    async fn test_vm_if_else_expression() {
        let code = r#"
            fn test(x: Boolean): String {
                return if x { "yes" } else { "no" }
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let mut context = Context::with_runtime(runtime.clone());

        context.declare_variable(
            "x".to_string(),
            crate::runtime::ExpressionResult::new(ExpressionValue::boolean(true)),
        );

        let vm = VM::new(runtime);
        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.as_string().unwrap(), "yes");
    }

    #[tokio::test]
    async fn test_vm_context_events() {
        let code = r#"
            fn test(): () {
                "event1"!
                "event2"!
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (returned_context, _result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(returned_context.events_count(), 2);
    }

    #[tokio::test]
    async fn test_vm_while_loop() {
        let code = r#"
            fn test(): Boolean {
                let x = false
                while false {
                    x = true
                }
                return x
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        assert_eq!(result.value.as_boolean().unwrap(), false);
    }

    #[tokio::test]
    async fn test_vm_nested_contexts() {
        let code = r#"
            fn test(): () {
                let x = "outer"
                if true {
                    let y = "inner"
                }
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (returned_context, _result) = vm.execute(&compiled, context).await.unwrap();
        assert!(returned_context.get_variable("x").is_some());
        assert!(returned_context.get_variable("y").is_none());
    }

    #[tokio::test]
    async fn test_vm_error_variable_not_found() {
        let code = r#"
            fn test(): String {
                return nonexistent
            }
        "#;

        let ast_module = parse_code(code);
        let ast_func = get_ast_function(&ast_module, "test");
        let typed_func = ast_func_to_typed(ast_func);
        let compiled = BytecodeCompiler::new()
            .compile_to_bytecode(&typed_func)
            .unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Variable not found"));
    }

    #[tokio::test]
    async fn test_vm_error_type_mismatch_brfalse() {
        let code = r#"
            fn test(): () {
                let x = "not a boolean"
                if x {
                    "unreachable"!
                }
            }
        "#;

        let ast_module = parse_code(code);
        let ast_func = get_ast_function(&ast_module, "test");
        let typed_func = ast_func_to_typed(ast_func);
        let compiled = BytecodeCompiler::new()
            .compile_to_bytecode(&typed_func)
            .unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected boolean"));
    }

    #[tokio::test]
    async fn test_vm_drop_removes_variable() {
        let code = r#"
            fn test(): () {
                let x = "temp"
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (returned_context, _result) = vm.execute(&compiled, context).await.unwrap();
        assert!(returned_context.get_variable("$tmp0").is_none());
    }

    #[tokio::test]
    async fn test_vm_decl_creates_unit_variable() {
        let code = r#"
            fn test(): () {
                let x = "value"
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime = Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (returned_context, _result) = vm.execute(&compiled, context).await.unwrap();
        let x_value = returned_context.get_variable("x").unwrap();
        assert_eq!(x_value.value.as_string().unwrap(), "value");
    }

    #[tokio::test]
    async fn test_vm_function_call() {
        let code = r#"
            fn helper(x: String): String {
                return x
            }

            fn test(): String {
                return helper("test_value")
            }
        "#;

        let program = CompilationUnit::from_string(code.to_string());
        let compiler = crate::compiler::Compiler::new();
        let compiled_program = compiler.compile_source(&program).unwrap();

        let test_compiled = compiled_program
            .resolve(&FunctionName {
                name: "test".to_string(),
                module: ModuleName::from_str("main"),
                kind: FunctionNameKind::Function,
            })
            .unwrap()
            .clone();

        let mut runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();

        for function in compiled_program.functions().values() {
            runtime.register_function(Box::new(BytecodeFunctionExpr::new(function.clone())));
        }

        let runtime = Arc::new(runtime);
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let result = vm.execute(&test_compiled, context).await.unwrap();
        assert_eq!(result.1.value.as_string().unwrap(), "test_value");
    }
}

#[cfg(test)]
mod struct_bytecode_tests {
    use crate::bytecode::BytecodeCompiler;
    use crate::cli::config::ProgramSource;
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;
    use crate::runtime::{ExpressionValue, Runtime};
    use crate::typecheck::TypeChecker;
    use crate::typecheck::checker::TypedCheckerAstRef;
    use crate::typed_ast;
    use arrow::array::Array;
    use std::collections::HashMap;

    fn parse_code(code: &str) -> crate::ast::Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new();
        parser.parse(&unit, file_id, manager.reporter()).unwrap()
    }

    fn parse_and_typecheck(code: &str) -> typed_ast::Module {
        let module = parse_code(code);
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parsed = crate::ast::ParsedModule {
            name: "".to_string(),
            module,
            is_entry: false,
            file_id,
        };
        let (_, typed_metadata) = TypeChecker::new()
            .check_modules(&[parsed], &HashMap::new())
            .unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module.to_string() != "" {
                    return None;
                }
                if let TypedCheckerAstRef::Function(func, _) = &f.ast_ref {
                    Some(typed_ast::Definition::Function((**func).clone()))
                } else {
                    None
                }
            })
            .collect();
        typed_ast::Module {
            definitions,
            span: crate::types::Span::dummy(),
            file_id,
        }
    }

    fn get_function<'a>(module: &'a typed_ast::Module, name: &str) -> &'a typed_ast::Function {
        for def in &module.definitions {
            if let typed_ast::Definition::Function(f) = def {
                if f.name == name {
                    return f;
                }
            }
        }
        panic!("Function '{}' not found", name);
    }

    #[test]
    fn test_struct_literal_compiles_to_struct_new() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn make(): Point {
    return Point { x: 1, y: 2 }
}
"#;
        let module = parse_and_typecheck(code);
        let func = get_function(&module, "make");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();
        let has_struct_new = compiled
            .instructions
            .iter()
            .any(|i| matches!(i, crate::bytecode::Instruction::StructNew { struct_name, .. } if struct_name == "Point"));
        assert!(has_struct_new, "Expected StructNew instruction for Point");
    }

    #[test]
    fn test_field_access_compiles_to_struct_get() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn get_x(p: Point): Int {
    return p.x
}
"#;
        let module = parse_and_typecheck(code);
        let func = get_function(&module, "get_x");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();
        let has_struct_get = compiled.instructions.iter().any(
            |i| matches!(i, crate::bytecode::Instruction::StructGet { field, .. } if field == "x"),
        );
        assert!(has_struct_get, "Expected StructGet instruction for field x");
    }

    #[tokio::test]
    async fn test_vm_struct_construction_and_field_access() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn make_point(): Int {
    let p = Point { x: 42, y: 7 }
    return p.x
}
fn main(): Int {
    return make_point()
}
"#;
        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        let result = runtime.run().await.unwrap();
        assert_eq!(result.as_integer().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_vm_struct_string_field() {
        let code = r#"
struct Task {
    title: String,
}
fn get_title(): String {
    let t = Task { title: "buy milk" }
    return t.title
}
fn main(): String {
    return get_title()
}
"#;
        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        let result = runtime.run().await.unwrap();
        assert_eq!(result.as_string().unwrap(), "buy milk");
    }

    #[tokio::test]
    async fn test_vm_struct_passed_as_argument() {
        let code = r#"
struct Point {
    x: Int,
    y: Int,
}
fn get_y(p: Point): Int {
    return p.y
}
fn main(): Int {
    let p = Point { x: 1, y: 99 }
    return get_y(p)
}
"#;
        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        let result = runtime.run().await.unwrap();
        assert_eq!(result.as_integer().unwrap(), 99);
    }

    #[test]
    fn test_struct_value_type_name() {
        let value = ExpressionValue::struct_value(vec![
            ("x", ExpressionValue::integer(1)),
            ("y", ExpressionValue::integer(2)),
        ]);
        assert_eq!(value.type_name(), "Struct");
    }

    #[test]
    fn test_struct_value_get_field() {
        let value = ExpressionValue::struct_value(vec![
            ("x", ExpressionValue::integer(10)),
            ("y", ExpressionValue::integer(20)),
        ]);
        let x = value.get_struct_field("x").unwrap();
        assert_eq!(x.as_integer().unwrap(), 10);
        let y = value.get_struct_field("y").unwrap();
        assert_eq!(y.as_integer().unwrap(), 20);
    }

    #[test]
    fn test_struct_value_unknown_field_is_error() {
        let value = ExpressionValue::struct_value(vec![("x", ExpressionValue::integer(1))]);
        assert!(value.get_struct_field("z").is_err());
    }

    #[test]
    fn test_struct_value_format_for_llm() {
        let value =
            ExpressionValue::struct_value(vec![("title", ExpressionValue::string("hello"))]);
        let formatted = value.format_for_llm();
        assert!(formatted.contains("title"));
        assert!(formatted.contains("hello"));
    }

    #[test]
    fn test_struct_value_format_for_llm_int_and_bool_fields() {
        let value = ExpressionValue::struct_value(vec![
            ("count", ExpressionValue::integer(42)),
            ("active", ExpressionValue::boolean(true)),
        ]);
        let formatted = value.format_for_llm();
        assert!(formatted.contains("count"));
        assert!(formatted.contains("42"));
        assert!(formatted.contains("active"));
        assert!(formatted.contains("true"));
    }

    #[test]
    fn test_metadata_type_name_is_metadata_not_struct() {
        let value = ExpressionValue::metadata("my_func", Some("does things".to_string()));
        assert_eq!(value.type_name(), "Metadata");
    }

    #[test]
    fn test_user_struct_with_name_and_documentation_fields_is_not_metadata() {
        let value = ExpressionValue::struct_value(vec![
            ("name", ExpressionValue::string("alice")),
            ("documentation", ExpressionValue::string("some doc")),
        ]);
        assert_eq!(value.type_name(), "Struct");
    }

    #[test]
    fn test_from_elements_struct_list() {
        let a = ExpressionValue::struct_value(vec![
            ("x", ExpressionValue::integer(1)),
            ("y", ExpressionValue::integer(2)),
        ]);
        let b = ExpressionValue::struct_value(vec![
            ("x", ExpressionValue::integer(3)),
            ("y", ExpressionValue::integer(4)),
        ]);
        let list = ExpressionValue::from_elements(vec![a, b]).unwrap();
        assert_eq!(list.type_name(), "List");
        let arr = list.as_list().unwrap();
        assert_eq!(arr.len(), 1);
        let values = arr.value(0);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_from_elements_option_list() {
        let a = ExpressionValue::option_some(ExpressionValue::string("hello"));
        let b = ExpressionValue::option_none();
        let list = ExpressionValue::from_elements(vec![a, b]).unwrap();
        assert_eq!(list.type_name(), "List");
        let arr = list.as_list().unwrap();
        assert_eq!(arr.len(), 1);
        let values = arr.value(0);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_format_for_llm_struct_list() {
        let a = ExpressionValue::struct_value(vec![("name", ExpressionValue::string("alice"))]);
        let b = ExpressionValue::struct_value(vec![("name", ExpressionValue::string("bob"))]);
        let list = ExpressionValue::from_elements(vec![a, b]).unwrap();
        let formatted = list.format_for_llm();
        assert!(formatted.contains("alice"), "got: {}", formatted);
        assert!(formatted.contains("bob"), "got: {}", formatted);
        assert!(formatted.contains("name"), "got: {}", formatted);
    }

    #[test]
    fn test_format_for_llm_option_list() {
        let a = ExpressionValue::option_some(ExpressionValue::string("hello"));
        let b = ExpressionValue::option_none();
        let list = ExpressionValue::from_elements(vec![a, b]).unwrap();
        let formatted = list.format_for_llm();
        assert!(formatted.contains("Some"), "got: {}", formatted);
        assert!(formatted.contains("hello"), "got: {}", formatted);
        assert!(formatted.contains("None"), "got: {}", formatted);
    }
}
