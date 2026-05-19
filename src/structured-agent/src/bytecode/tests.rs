#[cfg(test)]
mod instruction_display_tests {
    use crate::bytecode::Instruction;
    use nonempty::NonEmpty;
    use structured_agent_runtime::DefinitionPath;

    #[test]
    fn test_ldc_str_display() {
        use structured_agent_il::Slot;
        let instr = Instruction::LdcStr {
            dest: Slot(0),
            value: "hello".to_string(),
        };
        assert_eq!(format!("{}", instr), "ldc.str s0, \"hello\"");
    }

    #[test]
    fn test_call_bytecode_display() {
        use structured_agent_il::Slot;
        let instr = Instruction::CallBytecode {
            function_name: DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("mymod".to_string())),
                "foo",
            ),
            params: vec![Slot(0), Slot(1)],
            dest: Slot(2),
        };
        assert_eq!(
            format!("{}", instr),
            "call.bytecode mymod::foo, [s0, s1], s2"
        );
    }

    #[test]
    fn test_call_external_display() {
        use structured_agent_il::Slot;
        let instr = Instruction::CallExternal {
            function_name: DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("mymod".to_string())),
                "foo",
            ),
            params: vec![Slot(0), Slot(1)],
            dest: Slot(2),
        };
        assert_eq!(
            format!("{}", instr),
            "call.external mymod::foo, [s0, s1], s2"
        );
    }

    #[test]
    fn test_br_display() {
        let instr = Instruction::Br { offset: 5 };
        assert_eq!(format!("{}", instr), "br 5");
    }

    #[test]
    fn test_ctx_child_display() {
        let instr = Instruction::CtxChild;
        assert_eq!(format!("{}", instr), "ctx.child");
    }

    #[test]
    fn test_struct_new_display() {
        use structured_agent_il::Slot;
        use structured_agent_runtime::symbols::DefinitionPath;
        let instr = Instruction::StructNew {
            dest: Slot(0),
            struct_name: DefinitionPath::root()
                .with_module("test".to_string())
                .with_type("Point".to_string()),
            fields: vec![("x".to_string(), Slot(1)), ("y".to_string(), Slot(2))],
        };
        assert_eq!(
            format!("{}", instr),
            "struct.new s0, test::Point, {x: s1, y: s2}"
        );
    }

    #[test]
    fn test_struct_get_display() {
        use structured_agent_il::Slot;
        let instr = Instruction::StructGet {
            dest: Slot(0),
            src: Slot(1),
            field: "x".to_string(),
        };
        assert_eq!(format!("{}", instr), "struct.get s0, s1, x");
    }
}

#[cfg(test)]
mod compilation_tests {
    use crate::bytecode::BytecodeCompiler;
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;
    use crate::typecheck::TypeChecker;
    use crate::typecheck::TypedCheckerAstRef;
    use crate::typed_ast;
    use crate::types::Span;
    use nonempty::NonEmpty;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_il::Module as RuntimeModule;
    use structured_agent_runtime::Type as RT;
    use structured_agent_runtime::symbols::DefinitionPath;
    use structured_agent_stdlib::prelude::PreludeModule;

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
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id,
            is_inline: false,
        };
        let mut prelude = HashMap::new();
        prelude.insert(
            "prelude".to_string(),
            Arc::new(PreludeModule) as Arc<dyn RuntimeModule>,
        );
        let typed_metadata = TypeChecker::new().check(&[parsed], &prelude).unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module_prefix().to_string() != "test" {
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
            span: Span::dummy(),
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
    fn test_compile_match_expression() {
        let code = r#"
            fn test(m: Image | Audio): String {
                return match m { Image(img) => "image", Audio(audio) => "audio" }
            }
        "#;
        let expected = r#"fn test(
    m: Image | Audio
): String {
  .slots:
    s0  ret    $ret
    s1  param  m
    s2  local  img
    s3  local  audio
    s4  temp   $t0
    s5  temp   $t1
    s6  temp   $t2
      0: mov s5 ($t1), s1 (m)
      1: match.type s6 ($t2), s5 ($t1), prelude::Image
      2: brfalse s6 ($t2), 6
      3: mov s2 (img), s5 ($t1)
      4: ldc.str s4 ($t0), "image"
      5: br 11
  match_arm_0_1:
      6: nop
      7: match.type s6 ($t2), s5 ($t1), prelude::Audio
      8: mov s3 (audio), s5 ($t1)
      9: ldc.str s4 ($t0), "audio"
     10: br 11
  match_end_0:
     11: nop
     12: ret s4 ($t0)
}
"#;
        compile_and_check(code, expected);
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
        add(x, y),
        subtract(x, y)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
      0: ldc.str s1 ($t0), "hello"
      1: ret s1 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
      0: ldc.bool s1 ($t0), true
      1: ret s1 ($t0)
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

): Unit {
  .slots:
    s0  ret    $ret
    s1  local  x
    s2  temp   $t0
      0: ldc.str s1 (x), "test"
      1: ldc.unit s2 ($t0)
      2: ret s2 ($t0)
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

): Unit {
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
      0: ldc.str s1 ($t0), "event"
      1: ctx.event s1 ($t0)
      2: ldc.unit s2 ($t1)
      3: ret s2 ($t1)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
      0: ldc.str s2 ($t1), "arg1"
      1: ldc.bool s3 ($t2), true
      2: call.external test::foo, [s2 ($t1), s3 ($t2)], s1 ($t0)
      3: ret s1 ($t0)
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

): Unit {
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
    s4  temp   $t3
      0: ldc.bool s1 ($t0), true
      1: brfalse s1 ($t0), 7
      2: ctx.child
      3: ldc.str s2 ($t1), "then"
      4: ctx.event s2 ($t1)
      5: ctx.restore
      6: br 11
  else_0:
      7: ctx.child
      8: ldc.str s3 ($t2), "else"
      9: ctx.event s3 ($t2)
     10: ctx.restore
  end_0:
     11: nop
     12: ldc.unit s4 ($t3)
     13: ret s4 ($t3)
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

): Unit {
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
  loop_start_0:
      0: ldc.bool s1 ($t0), true
      1: brfalse s1 ($t0), 7
      2: ctx.child
      3: ldc.str s2 ($t1), "loop"
      4: ctx.event s2 ($t1)
      5: ctx.restore
      6: br 0
  loop_end_0:
      7: nop
      8: ldc.unit s3 ($t2)
      9: ret s3 ($t2)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
      0: ldc.str s2 ($t1), "a"
      1: ldc.str s3 ($t2), "b"
      2: list.create s1 ($t0), [s2 ($t1), s3 ($t2)]
      3: ret s1 ($t0)
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
): Unit {
  .slots:
    s0  ret    $ret
    s1  param  name
    s2  local  message
    s3  temp   $t0
    s4  temp   $t1
      0: ldc.str s2 (message), "Hello"
      1: mov s3 ($t0), s2 (message)
      2: ctx.event s3 ($t0)
      3: ldc.unit s4 ($t1)
      4: ret s4 ($t1)
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
  .slots:
    s0  ret    $ret
    s1  param  x
    s2  param  y
    s3  local  result
    s4  temp   $t0
    s5  temp   $t1
      0: mov s4 ($t0), s1 (x)
      1: call.external test::process, [s4 ($t0)], s3 (result)
      2: mov s5 ($t1), s3 (result)
      3: ret s5 ($t1)
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
  .slots:
    s0  ret    $ret
    s1  param  items
    s2  param  filter
    s3  local  result
    s4  temp   $t0
    s5  temp   $t1
    s6  temp   $t2
    s7  temp   $t3
    s8  temp   $t4
      0: ldc.str s3 (result), "initial"
      1: mov s4 ($t0), s2 (filter)
      2: brfalse s4 ($t0), 10
      3: ctx.child
      4: mov s5 ($t1), s1 (items)
      5: call.external test::transform, [s5 ($t1)], s3 (result)
      6: mov s6 ($t2), s3 (result)
      7: ctx.event s6 ($t2)
      8: ctx.restore
      9: br 14
  else_0:
     10: ctx.child
     11: ldc.str s7 ($t3), "skipped"
     12: ctx.event s7 ($t3)
     13: ctx.restore
  end_0:
     14: nop
     15: mov s8 ($t4), s3 (result)
     16: ret s8 ($t4)
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
                    analyze("code"),
                    summarize("text")
                }
            }
        "#;

        let expected = r#"fn test(

): String {
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
    s4  temp   $t3
    s5  temp   $t4
    s6  temp   $t5
  select_start_0:
      0: meta.function test::analyze, s2 ($t1)
      1: meta.function test::summarize, s3 ($t2)
      2: llm.select [s2 ($t1), s3 ($t2)], s4 ($t3)
      3: switch s4 ($t3), [4, 7]
  clause_0_1:
      4: ldc.str s5 ($t4), "code"
      5: call.external test::analyze, [s5 ($t4)], s1 ($t0)
      6: br 10
  clause_1_2:
      7: ldc.str s6 ($t5), "text"
      8: call.external test::summarize, [s6 ($t5)], s1 ($t0)
      9: br 10
  select_end_3:
     10: nop
     11: ret s1 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  param  x
    s2  temp   $t0
    s3  temp   $t1
      0: mov s3 ($t1), s1 (x)
      1: brfalse s3 ($t1), 4
      2: ldc.str s2 ($t0), "yes"
      3: br 5
  ifelse_else_0:
      4: ldc.str s2 ($t0), "no"
  ifelse_end_0:
      5: nop
      6: ret s2 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  local  x
    s2  temp   $t0
      0: ldc.str s1 (x), "initial"
      1: ldc.str s1 (x), "updated"
      2: mov s2 ($t0), s1 (x)
      3: ret s2 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
      0: ldc.int s2 ($t1), 1
      1: ldc.int s3 ($t2), 2
      2: struct.new s1 ($t0), test::Point, {x: s2 ($t1), y: s3 ($t2)}
      3: ret s1 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  param  p
    s2  temp   $t0
    s3  temp   $t1
      0: mov s3 ($t1), s1 (p)
      1: struct.get s2 ($t0), s3 ($t1), x
      2: ret s2 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  param  p
    s2  temp   $t0
    s3  temp   $t1
    s4  temp   $t2
      0: mov s4 ($t2), s1 (p)
      1: struct.get s3 ($t1), s4 ($t2), address
      2: struct.get s2 ($t0), s3 ($t1), city
      3: ret s2 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
      0: call.bytecode test::make_point, [], s2 ($t1)
      1: struct.get s1 ($t0), s2 ($t1), x
      2: ret s1 ($t0)
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
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
      0: llm.placeholder s2 ($t1), foo, x, String
      1: call.external test::foo, [s2 ($t1)], s1 ($t0)
      2: ret s1 ($t0)
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

): Unit {
  .slots:
    s0  ret    $ret
    s1  temp   $t0
      0: ldc.unit s1 ($t0)
      1: ret s1 ($t0)
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

): Unit {
  .slots:
    s0  ret    $ret
    s1  local  x
    s2  temp   $t0
      0: ldc.unit s1 (x)
      1: mov s2 ($t0), s1 (x)
      2: ret s2 ($t0)
}
"#;
        compile_and_check(code, expected);
    }

    #[test]
    fn test_compile_string_template() {
        let func = typed_ast::Function {
            name: "test".to_string(),
            parameters: vec![],
            return_type: RT::string(),
            body: typed_ast::FunctionBody {
                statements: vec![typed_ast::Statement::Return(
                    typed_ast::Expression::StringTemplate {
                        parts: vec![
                            typed_ast::StringPart::Literal("hello ".to_string()),
                            typed_ast::StringPart::Interpolated(Box::new(
                                typed_ast::Expression::IntLiteral {
                                    value: 42,
                                    ty: RT::int(),
                                    span: Span::dummy(),
                                },
                            )),
                        ],
                        ty: RT::string(),
                        span: Span::dummy(),
                    },
                )],
                span: Span::dummy(),
            },
            documentation: None,
            is_pub: false,
            span: Span::dummy(),
        };
        let compiled = BytecodeCompiler::new().compile_to_bytecode(&func).unwrap();
        let expected = r#"fn test(

): String {
  .slots:
    s0  ret    $ret
    s1  temp   $t0
    s2  temp   $t1
    s3  temp   $t2
      0: ldc.str s2 ($t1), "hello "
      1: ldc.int s3 ($t2), 42
      2: str.concat s1 ($t0), [s2 ($t1), s3 ($t2)]
      3: ret s1 ($t0)
}
"#;
        assert_eq!(format!("{}", compiled), expected);
    }

    #[test]
    fn test_compile_if_let_statement() {
        let code = r#"
            fn test(m: Image | Audio): () {
                if let Image(img) = m {
                    "then"!
                } else {
                    "else"!
                }
            }
        "#;
        let expected = r#"fn test(
    m: Image | Audio
): Unit {
  .slots:
    s0  ret    $ret
    s1  param  m
    s2  local  img
    s3  temp   $t0
    s4  temp   $t1
    s5  temp   $t2
    s6  temp   $t3
    s7  temp   $t4
      0: mov s3 ($t0), s1 (m)
      1: match.type s4 ($t1), s3 ($t0), prelude::Image
      2: brfalse s4 ($t1), 9
      3: mov s2 (img), s3 ($t0)
      4: ctx.child
      5: ldc.str s5 ($t2), "then"
      6: ctx.event s5 ($t2)
      7: ctx.restore
      8: br 13
  else_0:
      9: ctx.child
     10: ldc.str s6 ($t3), "else"
     11: ctx.event s6 ($t3)
     12: ctx.restore
  end_0:
     13: nop
     14: ldc.unit s7 ($t4)
     15: ret s7 ($t4)
}
"#;
        compile_and_check(code, expected);
    }
}

#[cfg(test)]
mod vm_execution_tests {
    use crate::bytecode::{BytecodeCompiler, VM};
    use crate::cli::config::ProgramSource;
    use crate::compiler::{CodespanParser, CompilationUnit};
    use crate::diagnostics::DiagnosticManager;
    use crate::runtime::{Context, ExpressionValue, Runtime, RuntimeService};
    use crate::typecheck::TypeChecker;
    use crate::typecheck::TypedCheckerAstRef;
    use crate::typed_ast;
    use nonempty::NonEmpty;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_il::Module as RuntimeModule;
    use structured_agent_runtime::DefinitionPath;
    use structured_agent_stdlib::prelude::PreludeModule;

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
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: false,
            file_id,
            is_inline: false,
        };
        let mut prelude = HashMap::new();
        prelude.insert(
            "prelude".to_string(),
            Arc::new(PreludeModule) as Arc<dyn RuntimeModule>,
        );
        let typed_metadata = TypeChecker::new().check(&[parsed], &prelude).unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module_prefix().to_string() != "test" {
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

    fn ast_type_to_rt(t: &crate::ast::Type) -> structured_agent_runtime::Type {
        use crate::ast::Type as AstType;
        use nonempty::NonEmpty;
        use structured_agent_runtime::DefinitionPath;
        use structured_agent_runtime::Type as RT;
        match t {
            AstType::Union(_) => RT::unit(),
            AstType::Named { args, .. } if args.is_empty() => match t.name().as_str() {
                "Boolean" => RT::boolean(),
                "String" => RT::string(),
                "Int" => RT::int(),
                "Unit" => RT::unit(),
                _ => RT::Named(DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("main".to_string())),
                    t.name(),
                )),
            },
            AstType::Named { args, .. } => {
                let module_str = match t.name().as_str() {
                    "List" | "Option" => "prelude",
                    _ => "main",
                };
                RT::Parameterized(
                    DefinitionPath::for_type(
                        DefinitionPath::for_module(NonEmpty::new(module_str.to_string())),
                        t.name(),
                    ),
                    args.iter().map(ast_type_to_rt).collect(),
                )
            }
        }
    }

    fn ast_expr_to_typed(expr: &crate::ast::Expression) -> typed_ast::Expression {
        use crate::ast::Expression as AE;
        use structured_agent_runtime::Type;
        let _dummy = crate::types::Span::dummy();
        match expr {
            AE::StringLiteral { value, span } => typed_ast::Expression::StringLiteral {
                value: value.clone(),
                ty: Type::string(),
                span: *span,
            },
            AE::BooleanLiteral { value, span } => typed_ast::Expression::BooleanLiteral {
                value: *value,
                ty: Type::boolean(),
                span: *span,
            },
            AE::IntLiteral { value, span } => typed_ast::Expression::IntLiteral {
                value: *value,
                ty: Type::int(),
                span: *span,
            },
            AE::Variable { name, span } => typed_ast::Expression::Variable {
                name: name.clone(),
                binding_id: typed_ast::BindingId(0),
                ty: Type::unit(),
                span: *span,
            },
            AE::UnitLiteral { span } => typed_ast::Expression::UnitLiteral {
                ty: Type::unit(),
                span: *span,
            },
            AE::Placeholder { span } => typed_ast::Expression::Placeholder {
                ty: Type::unit(),
                param_name: String::new(),
                function_name: String::new(),
                span: *span,
            },
            AE::Call {
                function,
                arguments,
                span,
                ..
            } => typed_ast::Expression::Call {
                function: function.clone(),
                binding: typed_ast::MethodBinding::Early(DefinitionPath::for_function(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    function.to_string(),
                )),
                kind: crate::typecheck::FunctionKind::External,
                arguments: arguments.iter().map(ast_expr_to_typed).collect(),
                target: None,
                ty: Type::unit(),
                span: *span,
            },
            AE::ListLiteral { elements, span } => typed_ast::Expression::ListLiteral {
                elements: elements.iter().map(ast_expr_to_typed).collect(),
                ty: Type::unit(),
                span: *span,
            },
            AE::StructLiteral {
                struct_name,
                fields,
                span,
            } => typed_ast::Expression::StructLiteral {
                struct_name: DefinitionPath::for_type(
                    DefinitionPath::for_module(NonEmpty::new("test".to_string())),
                    struct_name.clone(),
                ),
                fields: fields
                    .iter()
                    .map(|(n, e)| (n.clone(), ast_expr_to_typed(e)))
                    .collect(),
                ty: Type::unit(),
                span: *span,
            },
            AE::FieldAccess { base, field, span } => typed_ast::Expression::FieldAccess {
                base: Box::new(ast_expr_to_typed(base)),
                field: field.clone(),
                ty: Type::unit(),
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
                ty: Type::unit(),
                span: *span,
            },
            AE::MethodCall { .. } => unimplemented!("MethodCall not supported in test helper"),
            AE::Spawn { .. } => unimplemented!("Spawn not supported in test helper"),
            AE::StringTemplate { .. } => unreachable!("StringTemplate not yet produced by parser"),
            AE::Select(select) => typed_ast::Expression::Select(
                typed_ast::SelectExpression {
                    clauses: select
                        .clauses
                        .iter()
                        .map(|c| typed_ast::SelectClause {
                            expression_to_run: ast_expr_to_typed(&c.expression_to_run),
                            span: c.span,
                        })
                        .collect(),
                    span: select.span,
                },
                structured_agent_runtime::Type::unit(),
            ),
            AE::Match { .. } => unimplemented!("Match not supported in test helper"),
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
                ..
            } => typed_ast::Statement::Assignment {
                variable: variable.clone(),
                binding_id: typed_ast::BindingId(0),
                expression: ast_expr_to_typed(expression),
                span: *span,
            },
            AS::VariableAssignment {
                variable,
                expression,
                span,
            } => typed_ast::Statement::VariableAssignment {
                variable: variable.clone(),
                binding_id: typed_ast::BindingId(0),
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
            AS::Yield { span } => typed_ast::Statement::Yield { span: *span },
            AS::ForIn { .. } => todo!("ForIn not yet supported in VM test helper"),
            AS::Match { .. } => unimplemented!("Match not supported in test helper"),
            AS::IfLet { .. } => unimplemented!("IfLet not supported in test helper"),
        }
    }

    fn ast_func_to_typed(f: &crate::ast::Function) -> typed_ast::Function {
        typed_ast::Function {
            name: f.name.clone(),
            parameters: f
                .parameters
                .iter()
                .map(|p| typed_ast::Parameter {
                    name: p.name.clone(),
                    param_type: ast_type_to_rt(&p.param_type),
                    binding_id: typed_ast::BindingId(0),
                    span: p.span,
                })
                .collect(),
            return_type: ast_type_to_rt(&f.return_type),
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());

        let mut frame = vec![None; compiled.slot_table.len()];
        frame[1] = Some(crate::runtime::ExpressionResult::new(
            ExpressionValue::boolean(true),
        ));

        let vm = VM::new(runtime);
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (returned_context, _result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_returned_context, _result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
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
        let result = BytecodeCompiler::new().compile_to_bytecode(&typed_func);
        assert!(result.is_err());
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let result = vm.execute(&compiled.instructions, context, frame).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Expected boolean"));
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

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; compiled.slot_table.len()];
        let (_returned_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
        assert_eq!(result.value.type_name(), "Unit");
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

        let test_body = compiled_program
            .metadata
            .functions
            .get(&DefinitionPath::for_function(
                DefinitionPath::for_module(NonEmpty::new("main".to_string())),
                "test",
            ))
            .and_then(|d| d.body_ref.as_ref())
            .unwrap()
            .clone();

        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        runtime.check().unwrap();

        let runtime: Arc<dyn RuntimeService> = Arc::new(runtime);
        let context = Context::with_runtime(runtime.clone());
        let vm = VM::new(runtime);

        let frame = vec![None; test_body.slot_table.len()];
        let result = vm
            .execute(&test_body.instructions, context, frame)
            .await
            .unwrap();
        assert_eq!(result.1.value.as_string().unwrap(), "test_value");
    }

    #[tokio::test]
    async fn test_vm_match_expression_image_arm() {
        let code = r#"
            fn test(m: Image | Audio): String {
                return match m { Image(img) => "image", Audio(audio) => "audio" }
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());

        let mut frame = vec![None; compiled.slot_table.len()];
        frame[1] = Some(crate::runtime::ExpressionResult::new(
            ExpressionValue::image("image/png", vec![]),
        ));

        let vm = VM::new(runtime);
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
        assert_eq!(result.value.as_string().unwrap(), "image");
    }

    #[tokio::test]
    async fn test_vm_match_expression_audio_arm() {
        let code = r#"
            fn test(m: Image | Audio): String {
                return match m { Image(img) => "image", Audio(audio) => "audio" }
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());

        let mut frame = vec![None; compiled.slot_table.len()];
        frame[1] = Some(crate::runtime::ExpressionResult::new(
            ExpressionValue::audio("audio/mp3", vec![]),
        ));

        let vm = VM::new(runtime);
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
        assert_eq!(result.value.as_string().unwrap(), "audio");
    }

    #[tokio::test]
    async fn test_vm_if_let_matching_branch_runs() {
        let code = r#"
            fn test(m: Image | Audio): String {
                if let Image(img) = m {
                    return "matched"
                }
                return "no_match"
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());

        let mut frame = vec![None; compiled.slot_table.len()];
        frame[1] = Some(crate::runtime::ExpressionResult::new(
            ExpressionValue::image("image/png", vec![]),
        ));

        let vm = VM::new(runtime);
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
        assert_eq!(result.value.as_string().unwrap(), "matched");
    }

    #[tokio::test]
    async fn test_vm_if_let_non_matching_skips_body() {
        let code = r#"
            fn test(m: Image | Audio): String {
                if let Image(img) = m {
                    return "matched"
                }
                return "no_match"
            }
        "#;

        let module = parse_and_typecheck(code);
        let func = get_function(&module, "test");
        let compiled = BytecodeCompiler::new().compile_to_bytecode(func).unwrap();

        let runtime: Arc<dyn RuntimeService> =
            Arc::new(Runtime::builder(ProgramSource::Inline("".to_string())).build());
        let context = Context::with_runtime(runtime.clone());

        let mut frame = vec![None; compiled.slot_table.len()];
        frame[1] = Some(crate::runtime::ExpressionResult::new(
            ExpressionValue::audio("audio/mp3", vec![]),
        ));

        let vm = VM::new(runtime);
        let (_context, result) = vm
            .execute(&compiled.instructions, context, frame)
            .await
            .unwrap();
        assert_eq!(result.value.as_string().unwrap(), "no_match");
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
    use crate::typecheck::TypedCheckerAstRef;
    use crate::typed_ast;
    use arrow::array::Array;
    use nonempty::NonEmpty;
    use std::collections::HashMap;
    use std::sync::Arc;
    use structured_agent_il::Module as RuntimeModule;
    use structured_agent_stdlib::prelude::PreludeModule;

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
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: false,
            file_id,
            is_inline: false,
        };
        let mut prelude = HashMap::new();
        prelude.insert(
            "prelude".to_string(),
            Arc::new(PreludeModule) as Arc<dyn RuntimeModule>,
        );
        let typed_metadata = TypeChecker::new().check(&[parsed], &prelude).unwrap();
        let definitions = typed_metadata
            .functions
            .values()
            .filter_map(|f| {
                if f.name.module_prefix().to_string() != "test" {
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
            .any(|i| matches!(i, crate::bytecode::Instruction::StructNew { struct_name, .. } if struct_name.last_name() == "Point"));
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

    #[tokio::test]
    async fn test_vm_generic_struct_field_access() {
        let code = r#"
struct Wrapper<T> {
    item: T,
}
fn get_item(): String {
    let w = Wrapper { item: "hello" }
    return w.item
}
fn main(): String {
    return get_item()
}
"#;
        let runtime = Runtime::builder(ProgramSource::Inline(code.to_string())).build();
        let result = runtime.run().await.unwrap();
        assert_eq!(result.as_string().unwrap(), "hello");
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
        let a = ExpressionValue::string("hello");
        let b = ExpressionValue::string("world");
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
        let a = ExpressionValue::string("hello");
        let b = ExpressionValue::string("world");
        let list = ExpressionValue::from_elements(vec![a, b]).unwrap();
        let formatted = list.format_for_llm();
        assert!(formatted.contains("hello"), "got: {}", formatted);
        assert!(formatted.contains("world"), "got: {}", formatted);
    }
}
