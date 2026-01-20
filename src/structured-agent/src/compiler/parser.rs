use crate::ast::{
    Expression, ExternalFunction, Function, FunctionBody, Parameter, SelectClause,
    SelectExpression, Statement, Type,
};
use combine::parser::char::{char, letter, spaces, string};
use combine::parser::choice::choice;
use combine::parser::repeat::{many, sep_by};
use combine::{Parser, Stream, attempt, between};

fn skip_spaces<Input>() -> impl Parser<Input, Output = ()>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    spaces().silent()
}

fn lex_char<Input>(c: char) -> impl Parser<Input, Output = char>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    char(c).skip(skip_spaces())
}

fn lex_string<Input>(s: &'static str) -> impl Parser<Input, Output = &'static str>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    string(s).skip(skip_spaces())
}

fn identifier<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        choice((letter(), char('_'))),
        many(choice((combine::parser::char::alpha_num(), char('_')))),
    )
        .map(|(first, rest): (char, Vec<char>)| {
            let mut result = String::new();
            result.push(first);
            result.extend(rest);
            result
        })
        .skip(skip_spaces())
}

pub fn parse_program<Input>() -> impl Parser<Input, Output = (Vec<Function>, Vec<ExternalFunction>)>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    skip_spaces()
        .with(many(choice((
            parse_function().map(|f| (Some(f), None)),
            parse_external_function().map(|ef| (None, Some(ef))),
        ))))
        .map(|items: Vec<(Option<Function>, Option<ExternalFunction>)>| {
            let mut functions = Vec::new();
            let mut external_functions = Vec::new();

            for (func, ext_func) in items {
                if let Some(f) = func {
                    functions.push(f);
                }
                if let Some(ef) = ext_func {
                    external_functions.push(ef);
                }
            }

            (functions, external_functions)
        })
}

fn parse_external_function<Input>() -> impl Parser<Input, Output = ExternalFunction>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        lex_string("extern"),
        lex_string("fn"),
        identifier(),
        between(
            lex_char('('),
            lex_char(')'),
            sep_by(parse_parameter(), lex_char(',')),
        ),
        lex_string("->"),
        parse_type(),
        lex_char(';'),
    )
        .map(|(_, _, name, params, _, return_type, _)| ExternalFunction {
            name,
            parameters: params,
            return_type,
        })
}

fn parse_function<Input>() -> impl Parser<Input, Output = Function>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        lex_string("fn"),
        identifier(),
        between(
            lex_char('('),
            lex_char(')'),
            sep_by(parse_parameter(), lex_char(',')),
        ),
        lex_string("->"),
        parse_type(),
        between(lex_char('{'), lex_char('}'), parse_function_body()),
    )
        .map(|(_, name, params, _, return_type, body)| Function {
            name,
            parameters: params,
            return_type,
            body,
        })
}

fn parse_parameter<Input>() -> impl Parser<Input, Output = Parameter>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (identifier(), lex_char(':'), parse_type())
        .map(|(name, _, param_type)| Parameter { name, param_type })
}

fn parse_type<Input>() -> impl Parser<Input, Output = Type>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        lex_string("()").map(|_| Type::Unit),
        lex_string("Boolean").map(|_| Type::Boolean),
        identifier().map(Type::Named),
    ))
}

fn parse_function_body<Input>() -> impl Parser<Input, Output = FunctionBody>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    many(statement().skip(skip_spaces())).map(|statements| FunctionBody { statements })
}

combine::parser! {
    fn statement[Input]()(Input) -> Statement
    where [Input: Stream<Token = char>]
    {
        choice((
            attempt(parse_assignment()),
            attempt(parse_variable_assignment()),
            attempt(parse_select()),
            attempt(parse_injection()),
            attempt(parse_if_statement()),
            attempt(parse_while_statement()),
            parse_expression_statement(),
        ))
    }
}

fn parse_injection<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    parse_expression()
        .skip(lex_char('!'))
        .map(Statement::Injection)
}

fn parse_assignment<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        lex_string("let"),
        identifier(),
        lex_char('='),
        parse_expression(),
    )
        .map(|(_, variable, _, expression)| Statement::Assignment {
            variable,
            expression,
        })
}

fn parse_variable_assignment<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (identifier(), lex_char('='), parse_expression()).map(|(variable, _, expression)| {
        Statement::VariableAssignment {
            variable,
            expression,
        }
    })
}

fn parse_expression_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    parse_expression().map(Statement::ExpressionStatement)
}

fn parse_expression<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        attempt(parse_call()),
        attempt(parse_select_expression()),
        parse_placeholder(),
        parse_string_literal(),
        parse_boolean_literal(),
        parse_variable(),
    ))
}

fn parse_call<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        attempt(
            (
                identifier(),
                lex_string("::"),
                identifier(),
                between(
                    lex_char('('),
                    lex_char(')'),
                    sep_by(parse_argument(), lex_char(',')),
                ),
            )
                .map(|(target, _, function, args)| Expression::Call {
                    target,
                    function,
                    arguments: args,
                    is_method: false,
                }),
        ),
        attempt(
            (
                identifier(),
                lex_char('.'),
                identifier(),
                between(
                    lex_char('('),
                    lex_char(')'),
                    sep_by(parse_argument(), lex_char(',')),
                ),
            )
                .map(|(target, _, function, args)| Expression::Call {
                    target,
                    function,
                    arguments: args,
                    is_method: true,
                }),
        ),
        (
            identifier(),
            between(
                lex_char('('),
                lex_char(')'),
                sep_by(parse_argument(), lex_char(',')),
            ),
        )
            .map(|(function, args)| Expression::Call {
                target: String::new(),
                function,
                arguments: args,
                is_method: false,
            }),
    ))
}

fn parse_argument<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        parse_placeholder(),
        parse_string_literal(),
        parse_boolean_literal(),
        parse_variable(),
    ))
}

fn parse_string_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    between(
        lex_char('"'),
        char('"'),
        many(combine::satisfy(|c| c != '"')),
    )
    .map(|chars: Vec<char>| Expression::StringLiteral(chars.into_iter().collect()))
}

fn parse_variable<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    identifier().map(Expression::Variable)
}

fn parse_placeholder<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    lex_char('_').map(|_| Expression::Placeholder)
}

fn parse_boolean_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        lex_string("true").map(|_| Expression::BooleanLiteral(true)),
        lex_string("false").map(|_| Expression::BooleanLiteral(false)),
    ))
}

fn parse_select<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    lex_string("select")
        .with(between(
            lex_char('{'),
            lex_char('}'),
            sep_by(parse_select_clause(), lex_char(',')),
        ))
        .map(|clauses| {
            Statement::ExpressionStatement(Expression::Select(SelectExpression { clauses }))
        })
}

fn parse_select_expression<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    lex_string("select")
        .with(between(
            lex_char('{'),
            lex_char('}'),
            sep_by(parse_select_clause(), lex_char(',')),
        ))
        .map(|clauses| Expression::Select(SelectExpression { clauses }))
}

fn parse_select_clause<Input>() -> impl Parser<Input, Output = SelectClause>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    parse_call()
        .skip(skip_spaces())
        .skip(lex_string("as"))
        .and(identifier())
        .skip(lex_string("=>"))
        .and(choice((
            attempt(parse_call()),
            parse_placeholder(),
            parse_string_literal(),
            parse_variable(),
        )))
        .map(
            |((expression_to_run, result_variable), expression_next)| SelectClause {
                expression_to_run,
                result_variable,
                expression_next,
            },
        )
}

fn parse_if_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        lex_string("if"),
        parse_expression(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(statement().skip(skip_spaces())),
        ),
    )
        .map(|(_, condition, body)| Statement::If { condition, body })
}

fn parse_while_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        lex_string("while"),
        parse_expression(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(statement().skip(skip_spaces())),
        ),
    )
        .map(|(_, condition, body)| Statement::While { condition, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::EasyParser;

    #[test]
    fn test_parse_simple_function() {
        let input = r#"
fn analyze_code(context: Context, code: String) -> Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}
"#;

        let result = parse_program().easy_parse(input);
        assert!(result.is_ok());

        let ((functions, external_functions), _) = result.unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(external_functions.len(), 0);

        let func = &functions[0];
        assert_eq!(func.name, "analyze_code");
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.parameters[0].name, "context");
        assert_eq!(func.parameters[1].name, "code");
        assert_eq!(func.body.statements.len(), 3);
    }

    #[test]
    fn test_complete_example_from_ideas() {
        let input = r#"
fn analyze_code(context: Context, code: String) -> Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}

fn suggest_fix(context: Context, analysis: Analysis) -> CodeFix {
    "Given this analysis, suggest a fix"!
    analysis!
}

fn main() -> () {
    let ctx = Context::new()
    let code = "def divide(a, b): return a / b"
    let analysis = ctx.analyze_code(code)
    let fix = ctx.suggest_fix(analysis)
}
"#;

        let result = parse_program().easy_parse(input);
        assert!(result.is_ok());

        let ((functions, external_functions), _) = result.unwrap();
        assert_eq!(functions.len(), 3);
        assert_eq!(external_functions.len(), 0);

        assert_eq!(functions[0].name, "analyze_code");
        assert_eq!(functions[1].name, "suggest_fix");
        assert_eq!(functions[2].name, "main");

        let main_func = &functions[2];
        assert_eq!(main_func.body.statements.len(), 4);
    }

    #[test]
    fn test_parse_prompt_injection() {
        let input = r#""Analyze the following code for potential bugs"!"#;
        let result = statement().easy_parse(input);
        assert!(result.is_ok());

        let (statement, _) = result.unwrap();
        match statement {
            Statement::Injection(Expression::StringLiteral(content)) => {
                assert_eq!(content, "Analyze the following code for potential bugs");
            }
            _ => panic!("Expected injection with string literal"),
        }
    }

    #[test]
    fn test_parse_variable_injection() {
        let input = "code!";
        let result = statement().easy_parse(input);
        assert!(result.is_ok());

        let (statement, _) = result.unwrap();
        match statement {
            Statement::Injection(Expression::Variable(var)) => {
                assert_eq!(var, "code");
            }
            _ => panic!("Expected injection with variable"),
        }
    }

    #[test]
    fn test_parse_assignment_with_method_call() {
        let input = "let analysis = ctx.analyze_code(code)";
        let result = statement().easy_parse(input);
        assert!(result.is_ok());

        let (statement, _) = result.unwrap();
        match statement {
            Statement::Assignment {
                variable,
                expression,
            } => {
                assert_eq!(variable, "analysis");
                match expression {
                    Expression::Call {
                        target,
                        function,
                        arguments,
                        is_method,
                    } => {
                        assert_eq!(target, "ctx");
                        assert_eq!(function, "analyze_code");
                        assert_eq!(arguments.len(), 1);
                        match &arguments[0] {
                            Expression::Variable(name) => assert_eq!(name, "code"),
                            _ => panic!("Expected variable argument"),
                        }
                        assert!(is_method);
                    }
                    _ => panic!("Expected call"),
                }
            }
            _ => panic!("Expected assignment"),
        }
    }

    #[test]
    fn test_parse_call_with_expression_arguments() {
        let input = r#"func("hello", var_name, "world")"#;
        let result = parse_expression().easy_parse(input);
        assert!(result.is_ok());

        let (expression, _) = result.unwrap();
        match expression {
            Expression::Call {
                target,
                function,
                arguments,
                is_method,
            } => {
                assert_eq!(target, "");
                assert_eq!(function, "func");
                assert_eq!(arguments.len(), 3);
                assert!(!is_method);

                match &arguments[0] {
                    Expression::StringLiteral(s) => assert_eq!(s, "hello"),
                    _ => panic!("Expected string literal"),
                }

                match &arguments[1] {
                    Expression::Variable(name) => assert_eq!(name, "var_name"),
                    _ => panic!("Expected variable"),
                }

                match &arguments[2] {
                    Expression::StringLiteral(s) => assert_eq!(s, "world"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected call expression"),
        }
    }

    #[test]
    fn test_mixed_functions_and_externals() {
        let input = r#"
extern fn add(x: String, y: String) -> String;
extern fn subtract(x: String, y: String) -> String;

fn calculator(request: String) -> String {
    "You are a calculator"!
    request!
}
"#;

        let result = parse_program().easy_parse(input);
        assert!(result.is_ok());

        let ((functions, external_functions), _) = result.unwrap();
        assert_eq!(functions.len(), 1);
        assert_eq!(external_functions.len(), 2);

        assert_eq!(external_functions[0].name, "add");
        assert_eq!(external_functions[1].name, "subtract");
        assert_eq!(functions[0].name, "calculator");
    }

    #[test]
    fn test_parse_standalone_expression_statement() {
        let input = r#"
fn test_function() -> () {
    let result = some_call()
    another_call()
    "final prompt"!
}
"#;

        let result = parse_program().easy_parse(input);
        assert!(result.is_ok());

        let ((functions, _), _) = result.unwrap();
        assert_eq!(functions.len(), 1);

        let func = &functions[0];
        assert_eq!(func.name, "test_function");
        assert_eq!(func.body.statements.len(), 3);

        match &func.body.statements[0] {
            Statement::Assignment {
                variable,
                expression,
            } => {
                assert_eq!(variable, "result");
                match expression {
                    Expression::Call { function, .. } => {
                        assert_eq!(function, "some_call");
                    }
                    _ => panic!("Expected call expression"),
                }
            }
            _ => panic!("Expected assignment statement"),
        }

        match &func.body.statements[1] {
            Statement::ExpressionStatement(Expression::Call { function, .. }) => {
                assert_eq!(function, "another_call");
            }
            _ => panic!("Expected standalone expression statement with call"),
        }

        match &func.body.statements[2] {
            Statement::Injection(Expression::StringLiteral(content)) => {
                assert_eq!(content, "final prompt");
            }
            _ => panic!("Expected injection statement"),
        }
    }

    #[test]
    fn test_parse_select_statement() {
        let input = r#"
fn calculator_agent(ctx: Context, request: String) -> i32 {
    "You are a calculator. Use the tools provided."!
    request!

    let result = select {
        add(ctx, _, _) as sum => sum,
        subtract(ctx, _, _) as diff => diff
    }

    result
}
"#;

        let result = parse_program().easy_parse(input);
        assert!(result.is_ok());

        let ((functions, _), _) = result.unwrap();
        assert_eq!(functions.len(), 1);

        let func = &functions[0];
        assert_eq!(func.name, "calculator_agent");
        assert_eq!(func.body.statements.len(), 4);

        let Statement::Assignment {
            variable,
            expression,
        } = &func.body.statements[2]
        else {
            panic!("Expected assignment statement");
        };
        assert_eq!(variable, "result");

        let Expression::Select(select_stmt) = expression else {
            panic!("Expected select expression");
        };
        assert_eq!(select_stmt.clauses.len(), 2);

        let first_clause = &select_stmt.clauses[0];
        assert_eq!(first_clause.result_variable, "sum");

        let Expression::Call {
            function,
            arguments,
            ..
        } = &first_clause.expression_to_run
        else {
            panic!("Expected call expression");
        };
        assert_eq!(function, "add");
        assert_eq!(arguments.len(), 3);
        assert!(matches!(arguments[1], Expression::Placeholder));
        assert!(matches!(arguments[2], Expression::Placeholder));

        let second_clause = &select_stmt.clauses[1];
        assert_eq!(second_clause.result_variable, "diff");

        let Expression::Call { function, .. } = &second_clause.expression_to_run else {
            panic!("Expected call expression");
        };
        assert_eq!(function, "subtract");
    }
}
