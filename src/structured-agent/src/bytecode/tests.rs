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
      0: ldc.bool $tmp0, true
      1: brfalse $tmp0, 6
      2: ctx.child false
      3: ldc.str $tmp3, "then"
      4: ctx.event $tmp3
      5: ctx.restore
      6: br 5
      7: ctx.child false
      8: ldc.str $tmp4, "else"
      9: ctx.event $tmp4
     10: ctx.restore
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
      0: ldc.bool $tmp2, true
      1: brfalse $tmp2, 6
      2: ctx.child false
      3: ldc.str $tmp3, "loop"
      4: ctx.event $tmp3
      5: ctx.restore
      6: br -6
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
      3: mov $tmp1, message
      4: ctx.event $tmp1
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
      6: mov $tmp2, result
      7: ret $tmp2
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
      3: mov $tmp1, filter
      4: brfalse $tmp1, 11
      5: ctx.child false
      6: call.begin transform
      7: mov $tmp5, items
      8: call.arg arg0, $tmp5
      9: call.invoke $tmp4
     10: mov result, $tmp4
     11: mov $tmp6, result
     12: ctx.event $tmp6
     13: ctx.restore
     14: br 5
     15: ctx.child false
     16: ldc.str $tmp7, "skipped"
     17: ctx.event $tmp7
     18: ctx.restore
     19: mov $tmp8, result
     20: ret $tmp8
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
      0: select.begin 2
      1: select.clause analyze 0
      2: select.clause summarize 0
      3: llm.select $tmp3
      4: switch $tmp3, [1, 11]
      5: ctx.child false
      6: call.begin analyze
      7: ldc.str $tmp6, "code"
      8: call.arg arg0, $tmp6
      9: call.invoke $tmp5
     10: decl result
     11: mov result, $tmp5
     12: mov $tmp0, result
     13: ctx.restore
     14: br 11
     15: ctx.child false
     16: call.begin summarize
     17: ldc.str $tmp8, "text"
     18: call.arg arg0, $tmp8
     19: call.invoke $tmp7
     20: decl summary
     21: mov summary, $tmp7
     22: mov $tmp0, summary
     23: ctx.restore
     24: br 1
     25: ret $tmp0
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
      1: brfalse $tmp1, 3
      2: ldc.str $tmp0, "yes"
      3: br 2
      4: ldc.str $tmp0, "no"
      5: ret $tmp0
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
      3: ldc.str $tmp1, "updated"
      4: mov x, $tmp1
      5: mov $tmp2, x
      6: ret $tmp2
}
"#;
        compile_and_check(code, expected);
    }
}
