#[cfg(test)]
mod instruction_display_tests {
    use crate::bytecode::Instruction;

    #[test]
    fn test_ldc_str_display() {
        let instr = Instruction::LdcStr {
            dest: "x".to_string(),
            value: "hello".to_string(),
        };
        assert_eq!(format!("{}", instr), "ldc.str x, \"hello\"");
    }

    #[test]
    fn test_call_display() {
        let instr = Instruction::Call {
            function_name: "foo".to_string(),
            params: vec!["x".to_string(), "y".to_string()],
            dest: "result".to_string(),
        };
        assert_eq!(format!("{}", instr), "call foo, [x, y], result");
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
}

#[cfg(test)]
mod compilation_tests {
    use crate::ast::Module;
    use crate::bytecode::BytecodeCompiler;
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;

    fn parse_code(code: &str) -> Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new();
        parser.parse(&unit, file_id, manager.reporter()).unwrap()
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
        let module = parse_code(code);

        for def in &module.definitions {
            if let crate::ast::Definition::Function(func) = def {
                let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();
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
        let module = parse_code(code);

        for def in &module.definitions {
            if let crate::ast::Definition::Function(func) = def {
                let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();
                println!("\n{}", compiled);
            }
        }
    }

    fn get_function<'a>(module: &'a Module, name: &str) -> &'a crate::ast::Function {
        for def in &module.definitions {
            if let crate::ast::Definition::Function(f) = def {
                if f.name == name {
                    return f;
                }
            }
        }
        panic!("Function '{}' not found in module", name);
    }

    fn compile_and_check(code: &str, expected: &str) {
        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();
        assert_eq!(format!("{}", compiled), expected);
    }

    fn compile_and_check_named(code: &str, function_name: &str, expected: &str) {
        let module = parse_code(code);
        let func = get_function(&module, function_name);
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();
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
      5: call foo, [$tmp1, $tmp2], $tmp0
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
      5: list.new $tmp0, Unknown
      6: list.add $tmp0, $tmp1
      7: list.add $tmp0, $tmp2
      8: list.finish $tmp0
      9: ret $tmp0
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
      3: call process, [$tmp1], $tmp0
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
     12: call transform, [$tmp6], $tmp5
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
     16: call analyze, [$tmp9], $tmp8
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
     26: call summarize, [$tmp11], $tmp10
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
    fn test_compile_placeholder() {
        let code = r#"
            fn test(): String {
                return foo(_)
            }
        "#;

        let expected = r#"fn test(

): String {
      0: decl $tmp0
      1: decl $tmp1
      2: llm.placeholder $tmp1, placeholder, Unknown
      3: call foo, [$tmp1], $tmp0
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
    use crate::ast::Module;
    use crate::bytecode::{BytecodeCompiler, VM};
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;
    use crate::runtime::{Context, ExpressionValue, Runtime};
    use std::sync::Arc;

    fn parse_code(code: &str) -> Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new();
        parser.parse(&unit, file_id, manager.reporter()).unwrap()
    }

    fn get_function<'a>(module: &'a Module, name: &str) -> &'a crate::ast::Function {
        for def in &module.definitions {
            if let crate::ast::Definition::Function(f) = def {
                if f.name == name {
                    return f;
                }
            }
        }
        panic!("Function '{}' not found in module", name);
    }

    #[tokio::test]
    async fn test_vm_string_literal() {
        let code = r#"
            fn test(): String {
                return "hello"
            }
        "#;

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected string value"),
        }
    }

    #[tokio::test]
    async fn test_vm_boolean_literal() {
        let code = r#"
            fn test(): Boolean {
                return true
            }
        "#;

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::Boolean(b) => assert_eq!(b, true),
            _ => panic!("Expected boolean value"),
        }
    }

    #[tokio::test]
    async fn test_vm_unit_literal() {
        let code = r#"
            fn test(): () {
                return ()
            }
        "#;

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::Unit => {}
            _ => panic!("Expected unit value"),
        }
    }

    #[tokio::test]
    async fn test_vm_assignment_and_variable() {
        let code = r#"
            fn test(): String {
                let x = "test"
                return x
            }
        "#;

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "test"),
            _ => panic!("Expected string value"),
        }
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "updated"),
            _ => panic!("Expected string value"),
        }
    }

    #[tokio::test]
    async fn test_vm_if_else_expression() {
        let code = r#"
            fn test(x: Boolean): String {
                return if x { "yes" } else { "no" }
            }
        "#;

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let mut context = Context::with_runtime(runtime.clone());

        context.declare_variable(
            "x".to_string(),
            crate::runtime::ExpressionResult::new(ExpressionValue::Boolean(true)),
        );

        let vm = VM::new(runtime);
        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "yes"),
            _ => panic!("Expected string value"),
        }
    }

    #[tokio::test]
    async fn test_vm_context_events() {
        let code = r#"
            fn test(): () {
                "event1"!
                "event2"!
            }
        "#;

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (_context, result) = vm.execute(&compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::Boolean(b) => assert_eq!(b, false),
            _ => panic!("Expected boolean value"),
        }
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
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

        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_to_bytecode(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Arc::new(Runtime::builder(program).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let (returned_context, _result) = vm.execute(&compiled, context).await.unwrap();
        let x_value = returned_context.get_variable("x").unwrap();
        match x_value.value {
            ExpressionValue::String(ref s) => assert_eq!(s, "value"),
            _ => panic!("Expected string value"),
        }
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

        let module = parse_code(code);
        let test_func = get_function(&module, "test");
        let test_compiled = BytecodeCompiler::compile_to_bytecode(test_func).unwrap();

        let program = CompilationUnit::from_string(code.to_string());
        let compiler = crate::compiler::Compiler::new();
        let compiled_program = compiler.compile_program(&program).unwrap();

        let mut runtime = Runtime::builder(program.clone()).build();

        for function in compiled_program.functions().values() {
            runtime.register_function(function.clone_executable());
        }

        let runtime = Arc::new(runtime);
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let result = vm.execute(&test_compiled, context).await.unwrap();
        match result.1.value {
            ExpressionValue::String(s) => assert_eq!(s, "test_value"),
            _ => panic!("Expected string value"),
        }
    }
}
