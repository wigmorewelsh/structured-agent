use crate::ast::{
    Definition, Expression, ExternalFunction, Function, FunctionBody, Module, Parameter,
    SelectClause, SelectExpression, Statement, Type,
};
use crate::types::{FileId, Span, Spanned};
use combine::parser::char::{char, letter, newline, spaces, string};
use combine::parser::choice::choice;
use combine::parser::repeat::{many, many1, sep_by};
use combine::parser::token::satisfy;
use combine::{Parser, Stream, attempt, between, optional, position};

fn skip_spaces<Input>() -> impl Parser<Input, Output = ()>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    spaces().silent()
}

fn lex_char<Input>(c: char) -> impl Parser<Input, Output = char>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    char(c).skip(skip_spaces())
}

fn lex_string<Input>(s: &'static str) -> impl Parser<Input, Output = &'static str>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    string(s).skip(skip_spaces())
}

fn comment_line<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (char('#'), many(satisfy(|c| c != '\n')), optional(newline())).map(
        |(_, content, _): (char, Vec<char>, Option<char>)| {
            content.into_iter().collect::<String>().trim().to_string()
        },
    )
}

fn parse_comments<Input>() -> impl Parser<Input, Output = Option<String>>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    optional(many1(comment_line()))
        .map(|comments: Option<Vec<String>>| comments.map(|lines| lines.join("\n")))
}

fn identifier_raw<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char, Position = usize>,
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
}

fn identifier<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    identifier_raw().skip(skip_spaces())
}

pub fn parse_program<Input>(file_id: FileId) -> impl Parser<Input, Output = Module>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        skip_spaces().with(many(choice((
            parse_function_with_docs().map(Definition::Function),
            parse_external_function().map(Definition::ExternalFunction),
        )))),
        position(),
    )
        .map(move |(start, definitions, end)| Module {
            definitions,
            span: Span::new(start, end),
            file_id,
        })
}

fn parse_external_function<Input>() -> impl Parser<Input, Output = ExternalFunction>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("extern"),
        lex_string("fn"),
        identifier(),
        between(
            lex_char('('),
            lex_char(')'),
            sep_by(parse_parameter(), lex_char(',')),
        ),
        lex_char(':'),
        parse_type(),
        position(),
    )
        .map(
            |(start, _, _, name, params, _, return_type, end)| ExternalFunction {
                name,
                parameters: params,
                return_type,
                span: Span::new(start, end),
            },
        )
}

fn parse_function_with_docs<Input>() -> impl Parser<Input, Output = Function>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (parse_comments(), parse_function()).map(|(doc, mut func)| {
        func.documentation = doc;
        func
    })
}

fn parse_function<Input>() -> impl Parser<Input, Output = Function>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("fn"),
        identifier(),
        between(
            lex_char('('),
            lex_char(')'),
            sep_by(parse_parameter(), lex_char(',')),
        ),
        lex_char(':'),
        parse_type(),
        between(lex_char('{'), lex_char('}'), parse_function_body()),
        position(),
    )
        .map(
            |(start, _, name, params, _, return_type, body, end)| Function {
                name,
                parameters: params,
                return_type,
                body,
                documentation: None,
                span: Span::new(start, end),
            },
        )
}

fn parse_parameter<Input>() -> impl Parser<Input, Output = Parameter>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        identifier(),
        lex_char(':'),
        parse_type(),
        position(),
    )
        .map(|(start, name, _, param_type, end)| Parameter {
            name,
            param_type,
            span: Span::new(start, end),
        })
}

fn parse_type<Input>() -> impl Parser<Input, Output = Type>
where
    Input: Stream<Token = char, Position = usize>,
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
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        many(statement().skip(skip_spaces())),
        position(),
    )
        .map(|(start, statements, end)| FunctionBody {
            statements,
            span: Span::new(start, end),
        })
}

combine::parser! {
    fn statement[Input]()(Input) -> Statement
    where [Input: Stream<Token = char, Position = usize>]
    {
        choice((
            parse_assignment(),
            parse_variable_assignment(),
            attempt(parse_select()),
            attempt(parse_injection()),
            attempt(parse_if_statement()),
            attempt(parse_while_statement()),
            attempt(parse_return_statement()),
            parse_expression_statement(),
        ))
    }
}

fn parse_injection<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    parse_expression()
        .skip(lex_char('!'))
        .map(Statement::Injection)
}

fn parse_assignment<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        attempt(lex_string("let")),
        identifier(),
        lex_char('='),
        parse_expression(),
    )
        .map(|(start, _, variable, _, expression)| {
            let end = expression.span().end;
            Statement::Assignment {
                variable,
                expression,
                span: Span::new(start, end),
            }
        })
        .skip(skip_spaces())
}

fn parse_variable_assignment<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        attempt((identifier(), lex_char('='))),
        parse_expression(),
    )
        .skip(skip_spaces())
        .map(|(start, (variable, _), expression)| {
            let end = expression.span().end;
            Statement::VariableAssignment {
                variable,
                expression,
                span: Span::new(start, end),
            }
        })
}

fn parse_expression_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    parse_expression().map(Statement::ExpressionStatement)
}

combine::parser! {
    fn parse_simple_expression[Input]()(Input) -> Expression
    where [Input: Stream<Token = char, Position = usize>]
    {
        choice((
            attempt(parse_call()),
            parse_string_literal(),
            attempt(parse_boolean_literal()),
            parse_variable(),
        ))
    }
}

combine::parser! {
    fn parse_expression[Input]()(Input) -> Expression
    where [Input: Stream<Token = char, Position = usize>]
    {
        choice((
            attempt(parse_select_expression()),
            attempt(parse_if_else_expression()),
            parse_simple_expression(),
        ))
    }
}

combine::parser! {
    fn parse_if_else_expression[Input]()(Input) -> Expression
    where [Input: Stream<Token = char, Position = usize>]
    {
        (
            position(),
            lex_string("if"),
            parse_simple_expression(),
            between(lex_char('{'), lex_char('}'), parse_expression()),
            lex_string("else"),
            between(lex_char('{'), lex_char('}'), parse_expression()),
            position(),
        )
            .map(
                |(start, _, condition, then_expr, _, else_expr, end)| Expression::IfElse {
                    condition: Box::new(condition),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                    span: Span::new(start, end),
                },
            )
    }
}

fn parse_call<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        identifier(),
        between(
            lex_char('('),
            char(')'),
            sep_by(parse_argument(), lex_char(',')),
        ),
        position(),
    )
        .skip(skip_spaces())
        .map(|(start, function, args, end)| Expression::Call {
            function,
            arguments: args,
            span: Span::new(start, end),
        })
}

fn parse_argument<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((parse_placeholder(), parse_simple_expression()))
}

fn parse_string_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        attempt(parse_multiline_string()),
        parse_single_line_string(),
    ))
}

fn parse_single_line_string<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        between(
            lex_char('"'),
            char('"'),
            many(
                char('\\')
                    .with(satisfy(|_| true))
                    .map(|c| match c {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        c => c,
                    })
                    .or(satisfy(|c: char| c != '"')),
            ),
        ),
        position(),
    )
        .skip(skip_spaces())
        .map(
            |(start, chars, end): (_, Vec<char>, _)| Expression::StringLiteral {
                value: chars.into_iter().collect(),
                span: Span::new(start, end),
            },
        )
}

fn parse_multiline_string<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        between(
            lex_string("'''"),
            string("'''"),
            many(
                char('\\')
                    .with(satisfy(|_| true))
                    .map(|c| match c {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        c => c,
                    })
                    .or(satisfy(|c: char| c != '\'')),
            ),
        ),
        position(),
    )
        .skip(skip_spaces())
        .map(
            |(start, chars, end): (_, Vec<char>, _)| Expression::StringLiteral {
                value: chars.into_iter().collect(),
                span: Span::new(start, end),
            },
        )
}

fn parse_variable<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (position(), identifier_raw(), position())
        .skip(skip_spaces())
        .map(|(start, name, end)| Expression::Variable {
            name,
            span: Span::new(start, end),
        })
}

fn parse_placeholder<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (position(), lex_char('_'), position()).map(|(start, _, end)| Expression::Placeholder {
        span: Span::new(start, end),
    })
}

fn parse_boolean_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    choice((
        (position(), string("true"), position())
            .skip(skip_spaces())
            .map(|(start, _, end)| Expression::BooleanLiteral {
                value: true,
                span: Span::new(start, end),
            }),
        (position(), string("false"), position())
            .skip(skip_spaces())
            .map(|(start, _, end)| Expression::BooleanLiteral {
                value: false,
                span: Span::new(start, end),
            }),
    ))
}

fn parse_select<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("select").with(between(
            lex_char('{'),
            lex_char('}'),
            sep_by(parse_select_clause(), lex_char(',')),
        )),
        position(),
    )
        .map(|(start, clauses, end)| {
            Statement::ExpressionStatement(Expression::Select(SelectExpression {
                clauses,
                span: Span::new(start, end),
            }))
        })
}

fn parse_select_expression<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("select").with(between(
            lex_char('{'),
            lex_char('}'),
            sep_by(parse_select_clause(), lex_char(',')),
        )),
        position(),
    )
        .map(|(start, clauses, end)| {
            Expression::Select(SelectExpression {
                clauses,
                span: Span::new(start, end),
            })
        })
}

fn parse_select_clause<Input>() -> impl Parser<Input, Output = SelectClause>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        parse_call()
            .skip(skip_spaces())
            .skip(lex_string("as"))
            .and(identifier())
            .skip(lex_string("=>"))
            .and(parse_expression()),
        position(),
    )
        .map(
            |(start, ((expression_to_run, result_variable), expression_next), end)| SelectClause {
                expression_to_run,
                result_variable,
                expression_next,
                span: Span::new(start, end),
            },
        )
}

fn parse_if_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("if"),
        parse_simple_expression(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(statement().skip(skip_spaces())),
        ),
        position(),
    )
        .map(|(start, _, condition, body, end)| Statement::If {
            condition,
            body,
            span: Span::new(start, end),
        })
}

fn parse_while_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("while"),
        parse_simple_expression(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(statement().skip(skip_spaces())),
        ),
        position(),
    )
        .map(|(start, _, condition, body, end)| Statement::While {
            condition,
            body,
            span: Span::new(start, end),
        })
}

fn parse_return_statement<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (lex_string("return"), parse_expression()).map(|(_, expression)| Statement::Return(expression))
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::Parser;
    use combine::stream::position::{IndexPositioner, Stream};

    const TEST_FILE_ID: FileId = 0;

    #[test]
    fn test_parse_simple_multiline_string() {
        let input = r#"'''hello'''"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_multiline_string().parse(stream);
        if let Err(ref e) = result {
            println!("Parse error: {:?}", e);
        }
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "hello");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_empty_multiline_string() {
        let input = r#"''''''"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_multiline_string().parse(stream);
        if let Err(ref e) = result {
            println!("Parse error: {:?}", e);
        }
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_multiline_string() {
        let input = r#"'''
This is a multiline
string with "quotes" inside
and multiple lines
'''"#;

        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (expr, _) = parse_multiline_string().parse(stream).unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert!(value.contains("This is a multiline"));
                assert!(value.contains("string with \"quotes\" inside"));
                assert!(value.contains("and multiple lines"));
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_multiline_string_with_escaped_quote() {
        let input = r#"'''He said \"hello\"'''"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_multiline_string().parse(stream);
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "He said \"hello\"");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_multiline_string_with_escaped_backslash() {
        let input = r#"'''path\\to\\file'''"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_multiline_string().parse(stream);
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "path\\to\\file");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_multiline_string_with_newline_escape() {
        let input = r#"'''line1\nline2'''"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_multiline_string().parse(stream);
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "line1\nline2");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_multiline_string_with_unescaped_quote_fails() {
        let input = r#"'''This has an unescaped ' quote'''"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_multiline_string().parse(stream);
        // This should fail because the parser stops at the unescaped single quote
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_single_line_string() {
        let input = r#""test \"string\"""#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_single_line_string().parse(stream);
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "test \"string\"");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_single_line_string_with_escaped_quote() {
        let input = r#""Hello \"quoted\" World""#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_single_line_string().parse(stream);
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "Hello \"quoted\" World");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_empty_multiline_string_minimal() {
        let input = r#""""""""#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_string_literal().parse(stream);
        assert!(result.is_ok());

        let (expr, _) = result.unwrap();
        match expr {
            Expression::StringLiteral { value, .. } => {
                assert_eq!(value, "");
            }
            _ => panic!("Expected StringLiteral"),
        }
    }

    #[test]
    fn test_parse_simple_function() {
        let input = r#"
fn analyze_code(context: Context, code: String): Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "analyze_code");
        assert_eq!(func.parameters.len(), 2);
        assert_eq!(func.parameters[0].name, "context");
        assert_eq!(func.parameters[1].name, "code");
        assert_eq!(func.body.statements.len(), 3);
    }

    #[test]
    fn test_complete_example_from_ideas() {
        let input = r#"
fn analyze_code(context: Context, code: String): Analysis {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}

fn suggest_fix(context: Context, analysis: Analysis): CodeFix {
    "Given this analysis, suggest a fix"!
    analysis!
}

fn main(): () {
    let code = "def divide(a, b): return a / b"
    let analysis = analyze_code(code)
    let fix = suggest_fix(analysis)
}
"#;

        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        assert_eq!(module.definitions.len(), 3);

        let functions: Vec<_> = module
            .definitions
            .iter()
            .filter_map(|def| match def {
                Definition::Function(f) => Some(f),
                _ => None,
            })
            .collect();
        assert_eq!(functions.len(), 3);

        assert_eq!(functions[0].name, "analyze_code");
        assert_eq!(functions[1].name, "suggest_fix");
        assert_eq!(functions[2].name, "main");

        let main_func = &functions[2];
        assert_eq!(main_func.body.statements.len(), 3);
    }

    #[test]
    fn test_parse_prompt_injection() {
        let input = r#""Analyze the following code for potential bugs"!"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (statement, _) = statement().parse(stream).unwrap();
        match statement {
            Statement::Injection(Expression::StringLiteral { value, .. }) => {
                assert_eq!(value, "Analyze the following code for potential bugs");
            }
            _ => panic!("Expected injection with string literal"),
        }
    }

    #[test]
    fn test_parse_variable_injection() {
        let input = "code!";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (statement, _) = statement().parse(stream).unwrap();
        match statement {
            Statement::Injection(Expression::Variable { name, .. }) => {
                assert_eq!(name, "code");
            }
            _ => panic!("Expected injection with variable"),
        }
    }

    #[test]
    fn test_parse_assignment_with_method_call() {
        let input = "let analysis = analyze_code(code)";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (statement, _) = statement().parse(stream).unwrap();
        match statement {
            Statement::Assignment {
                variable,
                expression,
                span: _,
            } => {
                assert_eq!(variable, "analysis");
                match expression {
                    Expression::Call {
                        function,
                        arguments,
                        span: _,
                    } => {
                        assert_eq!(function, "analyze_code");
                        assert_eq!(arguments.len(), 1);
                        match &arguments[0] {
                            Expression::Variable { name, .. } => assert_eq!(name, "code"),
                            _ => panic!("Expected variable as argument"),
                        }
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
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_expression().parse(stream);
        assert!(result.is_ok());

        let (expression, _) = result.unwrap();
        match expression {
            Expression::Call {
                function,
                arguments,
                span: _,
            } => {
                assert_eq!(function, "func");
                assert_eq!(arguments.len(), 3);

                match &arguments[0] {
                    Expression::StringLiteral { value, .. } => assert_eq!(value, "hello"),
                    _ => panic!("Expected string literal"),
                }
                match &arguments[1] {
                    Expression::Variable { name, .. } => assert_eq!(name, "var_name"),
                    _ => panic!("Expected variable"),
                }
                match &arguments[2] {
                    Expression::StringLiteral { value, .. } => assert_eq!(value, "world"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected call expression"),
        }
    }

    #[test]
    fn test_mixed_functions_and_externals() {
        let input = r#"
extern fn add(x: String, y: String): String
extern fn subtract(x: String, y: String): String

fn calculator(request: String): String {
    "You are a calculator"!
    request!
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 3);

        let functions: Vec<_> = module
            .definitions
            .iter()
            .filter_map(|def| match def {
                Definition::Function(f) => Some(f),
                _ => None,
            })
            .collect();
        let external_functions: Vec<_> = module
            .definitions
            .iter()
            .filter_map(|def| match def {
                Definition::ExternalFunction(ef) => Some(ef),
                _ => None,
            })
            .collect();
        assert_eq!(functions.len(), 1);
        assert_eq!(external_functions.len(), 2);

        assert_eq!(external_functions[0].name, "add");
        assert_eq!(external_functions[1].name, "subtract");
        assert_eq!(functions[0].name, "calculator");
    }

    #[test]
    fn test_parse_standalone_expression_statement() {
        let input = r#"
fn test_function(): () {
    let result = some_call()
    result
    another_call()
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "test_function");
        assert_eq!(func.body.statements.len(), 3);

        match &func.body.statements[0] {
            Statement::Assignment {
                variable,
                expression,
                span: _,
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
            Statement::ExpressionStatement(Expression::Variable { name, .. }) => {
                assert_eq!(name, "result");
            }
            _ => panic!("Expected expression statement with variable"),
        }

        match &func.body.statements[2] {
            Statement::ExpressionStatement(Expression::Call { function, .. }) => {
                assert_eq!(function, "another_call");
            }
            _ => panic!("Expected expression statement with call"),
        }
    }

    #[test]
    fn test_parse_select_statement() {
        let input = r#"
fn calculator_agent(ctx: Context, request: String): i32 {
    "You are a calculator. Use the tools provided."!
    request!

    let result = select {
        add(ctx, _, _) as sum => sum,
        subtract(ctx, _, _) as diff => diff
    }

    result
}
"#;

        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "calculator_agent");
        assert_eq!(func.body.statements.len(), 4);

        let Statement::Assignment {
            variable,
            expression,
            span: _,
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
        assert!(matches!(arguments[1], Expression::Placeholder { .. }));
        assert!(matches!(arguments[2], Expression::Placeholder { .. }));

        let second_clause = &select_stmt.clauses[1];
        assert_eq!(second_clause.result_variable, "diff");

        let Expression::Call { function, .. } = &second_clause.expression_to_run else {
            panic!("Expected call expression");
        };
        assert_eq!(function, "subtract");
    }

    #[test]
    fn test_parse_function_with_comments() {
        let input = r#"
# This function analyzes code for bugs
# It focuses on edge cases and error handling
fn analyze_code(context: Context, code: String): Analysis {
    let analysis = run_analysis(code)
    analysis
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "analyze_code");
        assert!(func.documentation.is_some());
        let doc = func.documentation.as_ref().unwrap();
        assert_eq!(
            doc,
            "This function analyzes code for bugs\nIt focuses on edge cases and error handling"
        );
    }

    #[test]
    fn test_parse_function_without_comments() {
        let input = r#"
fn simple_function(): () {
    "Hello"!
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "simple_function");
        assert!(func.documentation.is_none());
    }

    #[test]
    fn test_parse_single_line_comment() {
        let input = r#"
# Single line documentation
fn documented_function(): () {
    "test"!
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "documented_function");
        assert!(func.documentation.is_some());
        let doc = func.documentation.as_ref().unwrap();
        assert_eq!(doc, "Single line documentation");
    }

    #[test]
    fn test_parse_return_statement() {
        let input = r#"return "hello world""#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (statement, _) = statement().parse(stream).unwrap();
        match statement {
            Statement::Return(Expression::StringLiteral { value, .. }) => {
                assert_eq!(value, "hello world");
            }
            _ => panic!("Expected return statement with string literal"),
        }
    }

    #[test]
    fn test_parse_return_with_variable() {
        let input = "return result";
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let (stmt, _) = statement().parse(stream).unwrap();
        match stmt {
            Statement::Return(Expression::Variable { name, .. }) => {
                assert_eq!(name, "result");
            }
            _ => panic!("Expected return statement with variable"),
        }
    }

    #[test]
    fn test_multiline_function_signature() {
        let input = r#"
fn multiline_function(
    first_param: String,
    second_param: Context,
    third_param: Analysis
): Result {
    "Process data"!
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };
        assert_eq!(func.name, "multiline_function");
        assert_eq!(func.parameters.len(), 3);
        assert_eq!(func.parameters[0].name, "first_param");
        assert_eq!(func.parameters[1].name, "second_param");
        assert_eq!(func.parameters[2].name, "third_param");
    }

    #[test]
    fn test_multiline_external_function() {
        let input = r#"
extern fn external_multiline(
    param1: String,
    param2: String
): Result
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let external_func = match &module.definitions[0] {
            Definition::ExternalFunction(ef) => ef,
            _ => panic!("Expected external function definition"),
        };
        let ext = external_func;
        assert_eq!(ext.name, "external_multiline");
        assert_eq!(ext.parameters.len(), 2);
        assert_eq!(ext.parameters[0].name, "param1");
        assert_eq!(ext.parameters[1].name, "param2");
    }

    #[test]
    fn test_parse_if_else_expression() {
        let input = r#"
fn test_if_else(): String {
    let result = if true { "then branch" } else { "else branch" }
    return result
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };

        assert_eq!(func.name, "test_if_else");
        assert_eq!(func.body.statements.len(), 2);

        match &func.body.statements[0] {
            Statement::Assignment {
                variable,
                expression,
                ..
            } => {
                assert_eq!(variable, "result");
                match expression {
                    Expression::IfElse {
                        condition,
                        then_expr,
                        else_expr,
                        ..
                    } => {
                        match condition.as_ref() {
                            Expression::BooleanLiteral { value, .. } => {
                                assert_eq!(*value, true);
                            }
                            _ => panic!("Expected boolean literal condition"),
                        }
                        match then_expr.as_ref() {
                            Expression::StringLiteral { value, .. } => {
                                assert_eq!(value, "then branch");
                            }
                            _ => panic!("Expected string literal in then branch"),
                        }
                        match else_expr.as_ref() {
                            Expression::StringLiteral { value, .. } => {
                                assert_eq!(value, "else branch");
                            }
                            _ => panic!("Expected string literal in else branch"),
                        }
                    }
                    _ => panic!("Expected if-else expression"),
                }
            }
            _ => panic!("Expected assignment statement"),
        }
    }

    #[test]
    fn test_parse_nested_if_else() {
        let input = r#"
fn nested(): String {
    return if true { if false { "a" } else { "b" } } else { "c" }
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };

        match &func.body.statements[0] {
            Statement::Return(Expression::IfElse { then_expr, .. }) => match then_expr.as_ref() {
                Expression::IfElse { .. } => {}
                _ => panic!("Expected nested if-else"),
            },
            _ => panic!("Expected return with if-else"),
        }
    }

    #[test]
    fn test_parse_variable_starting_with_f() {
        let input = r#"
fn test(): String {
    let flag = true
    let result = if flag { "works" } else { "nope" }
    return result
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };

        assert_eq!(func.name, "test");
    }

    #[test]
    fn test_parse_variable_starting_with_t() {
        let input = r#"
fn test(): String {
    let temp = false
    let result = if temp { "yes" } else { "no" }
    return result
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };

        assert_eq!(func.name, "test");
    }

    #[test]
    fn test_parse_variables_filter_and_total() {
        let input = r#"
fn test(): String {
    let filter = true
    let total = false
    let result = if filter { "filtered" } else { "not filtered" }
    return result
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());

        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());

        let (module, _) = result.unwrap();
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function definition"),
        };

        assert_eq!(func.name, "test");
    }
}
