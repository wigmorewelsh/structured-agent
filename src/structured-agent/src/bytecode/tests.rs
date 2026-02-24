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
    fn test_call_begin_display() {
        let instr = Instruction::CallBegin {
            function_name: "foo".to_string(),
        };
        assert_eq!(format!("{}", instr), "call.begin foo");
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
    use crate::compiler::{CodespanParser, CompilationUnit, Parser};
    use crate::diagnostics::DiagnosticManager;

    fn parse_code(code: &str) -> Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new(manager.reporter().clone());
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

    fn compile_and_check(code: &str, expected: &str) {
        let module = parse_code(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::compile_function(func).unwrap();
        assert_eq!(format!("{}", compiled), expected);
    }

    fn compile_and_check_named(code: &str, function_name: &str, expected: &str) {
        let module = parse_code(code);
        let func = get_function(&module, function_name);
        let compiled = BytecodeCompiler::compile_function(func).unwrap();
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
      0: ldc.str $tmp0, "hello"
      1: ret $tmp0
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
      0: ldc.bool $tmp0, true
      1: ret $tmp0
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
      0: ldc.str $tmp0, "test"
      1: decl x
      2: mov x, $tmp0
      3: drop $tmp0
      4: ldc.unit $tmp1
      5: ret $tmp1
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
      0: ldc.str $tmp0, "event"
      1: ctx.event $tmp0
      2: drop $tmp0
      3: ldc.unit $tmp1
      4: ret $tmp1
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
      0: call.begin foo
      1: ldc.str $tmp1, "arg1"
      2: call.arg arg0, $tmp1
      3: ldc.bool $tmp2, true
      4: call.arg arg1, $tmp2
      5: call.invoke $tmp0
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
      0: ldc.bool $tmp1, true
      1: brfalse $tmp1, 8
      2: ctx.child false
      3: ldc.str $tmp4, "then"
      4: ctx.event $tmp4
      5: drop $tmp4
      6: ctx.restore
      7: br 13
  else_$tmp2:
      8: ctx.child false
      9: ldc.str $tmp5, "else"
     10: ctx.event $tmp5
     11: drop $tmp5
     12: ctx.restore
  end_$tmp3:
     13: nop
     14: ldc.unit $tmp6
     15: ret $tmp6
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
      0: ldc.bool $tmp2, true
      1: brfalse $tmp2, 8
      2: ctx.child false
      3: ldc.str $tmp3, "loop"
      4: ctx.event $tmp3
      5: drop $tmp3
      6: ctx.restore
      7: br 0
  loop_end_$tmp1:
      8: nop
      9: ldc.unit $tmp4
     10: ret $tmp4
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
      0: ldc.str $tmp1, "a"
      1: ldc.str $tmp2, "b"
      2: list.new $tmp0, Unknown
      3: list.add $tmp0, $tmp1
      4: list.add $tmp0, $tmp2
      5: list.finish $tmp0
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
      0: ldc.str $tmp0, "Hello"
      1: decl message
      2: mov message, $tmp0
      3: drop $tmp0
      4: mov $tmp1, message
      5: ctx.event $tmp1
      6: drop $tmp1
      7: ldc.unit $tmp2
      8: ret $tmp2
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
      0: call.begin process
      1: mov $tmp1, x
      2: call.arg arg0, $tmp1
      3: call.invoke $tmp0
      4: decl result
      5: mov result, $tmp0
      6: drop $tmp0
      7: mov $tmp2, result
      8: ret $tmp2
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
      0: ldc.str $tmp0, "initial"
      1: decl result
      2: mov result, $tmp0
      3: drop $tmp0
  if_start_$tmp1:
      4: mov $tmp2, filter
      5: brfalse $tmp2, 18
      6: ctx.child false
      7: call.begin transform
      8: mov $tmp6, items
      9: call.arg arg0, $tmp6
     10: call.invoke $tmp5
     11: mov result, $tmp5
     12: drop $tmp5
     13: mov $tmp7, result
     14: ctx.event $tmp7
     15: drop $tmp7
     16: ctx.restore
     17: br 23
  else_$tmp3:
     18: ctx.child false
     19: ldc.str $tmp8, "skipped"
     20: ctx.event $tmp8
     21: drop $tmp8
     22: ctx.restore
  end_$tmp4:
     23: nop
     24: mov $tmp9, result
     25: ret $tmp9
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
  select_start_$tmp1:
      0: select.begin 2
      1: select.clause analyze 0
      2: select.clause summarize 0
      3: llm.select $tmp4
      4: switch $tmp4, [5, 15]
  clause_0_$tmp2:
      5: ctx.child false
      6: call.begin analyze
      7: ldc.str $tmp7, "code"
      8: call.arg arg0, $tmp7
      9: call.invoke $tmp6
     10: decl result
     11: mov result, $tmp6
     12: mov $tmp0, result
     13: ctx.restore
     14: br 25
  clause_1_$tmp3:
     15: ctx.child false
     16: call.begin summarize
     17: ldc.str $tmp9, "text"
     18: call.arg arg0, $tmp9
     19: call.invoke $tmp8
     20: decl summary
     21: mov summary, $tmp8
     22: mov $tmp0, summary
     23: ctx.restore
     24: br 25
  select_end_$tmp5:
     25: nop
     26: ret $tmp0
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
      0: mov $tmp1, x
      1: brfalse $tmp1, 4
      2: ldc.str $tmp0, "yes"
      3: br 5
  ifelse_else_$tmp2:
      4: ldc.str $tmp0, "no"
  ifelse_end_$tmp3:
      5: nop
      6: ret $tmp0
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
      0: ldc.str $tmp0, "initial"
      1: decl x
      2: mov x, $tmp0
      3: drop $tmp0
      4: ldc.str $tmp1, "updated"
      5: mov x, $tmp1
      6: drop $tmp1
      7: mov $tmp2, x
      8: ret $tmp2
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
      0: call.begin foo
      1: llm.placeholder $tmp1, placeholder, Unknown
      2: call.arg arg0, $tmp1
      3: call.invoke $tmp0
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
      0: ldc.unit $tmp0
      1: ret $tmp0
      2: ldc.unit $tmp1
      3: ret $tmp1
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
      0: ldc.unit $tmp0
      1: decl x
      2: mov x, $tmp0
      3: drop $tmp0
      4: mov $tmp1, x
      5: ret $tmp1
      6: ldc.unit $tmp2
      7: ret $tmp2
}
"#;
        compile_and_check(code, expected);
    }
}

#[cfg(test)]
mod vm_execution_tests {
    use crate::ast::Module;
    use crate::bytecode::{BytecodeCompiler, VM};
    use crate::compiler::{CodespanParser, CompilationUnit, CompilerTrait, Parser};
    use crate::diagnostics::DiagnosticManager;
    use crate::runtime::{Context, ExpressionValue, Runtime};
    use std::rc::Rc;
    use std::sync::Arc;

    fn parse_code(code: &str) -> Module {
        let unit = CompilationUnit::from_string(code.to_string());
        let mut manager = DiagnosticManager::new();
        let file_id = manager.add_file("test.sa".to_string(), code.to_string());
        let parser = CodespanParser::new(manager.reporter().clone());
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));

        context.variables.insert(
            "x".to_string(),
            crate::runtime::ExpressionResult::new(ExpressionValue::Boolean(true)),
        );

        let vm = VM::new(runtime);
        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        vm.execute(&compiled, context.clone()).await.unwrap();
        assert_eq!(context.events_count(), 2);
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&compiled, context).await.unwrap();
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        vm.execute(&compiled, context.clone()).await.unwrap();
        assert!(context.variables.contains_key("x"));
        assert!(!context.variables.contains_key("y"));
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        vm.execute(&compiled, context.clone()).await.unwrap();
        assert!(!context.variables.contains_key("$tmp0"));
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
        let compiled = BytecodeCompiler::compile_function(func).unwrap();

        let program = CompilationUnit::from_string("".to_string());
        let runtime = Rc::new(Runtime::builder(program).build());
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        vm.execute(&compiled, context.clone()).await.unwrap();
        let x_value = context.variables.get("x").unwrap();
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
        let test_compiled = BytecodeCompiler::compile_function(test_func).unwrap();

        let program = CompilationUnit::from_string(code.to_string());
        let compiler = crate::compiler::Compiler::new();
        let compiled_program = compiler.compile_program(&program).unwrap();

        let mut runtime = Runtime::builder(program.clone()).build();

        for function in compiled_program.functions().values() {
            runtime.register_function(function.clone());
        }

        let runtime = Rc::new(runtime);
        let context = Arc::new(Context::with_runtime(runtime.clone()));
        let vm = VM::new(runtime);

        let result = vm.execute(&test_compiled, context).await.unwrap();
        match result.value {
            ExpressionValue::String(s) => assert_eq!(s, "test_value"),
            _ => panic!("Expected string value"),
        }
    }
}
