use std::sync::Arc;

use crate::ast::{
    AstSignature, AstTrait, AstTraitImpl, Definition, Expression, ExternalFunction, Function,
    FunctionBody, Module, ModuleParam, Parameter, SelectClause, SelectExpression, SigFunction,
    Statement, StructDefinition, StructField, Type, TypeParam,
};
use crate::types::{FileId, Span, Spanned};
use combine::parser::char::{char, letter, newline, spaces, string};
use combine::parser::choice::choice;
use combine::parser::repeat::{many, many1, sep_by, skip_many};
use combine::parser::token::satisfy;
use combine::{Parser, Stream, attempt, between, optional, position, sep_by1};
use nonempty::NonEmpty;

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

fn doc_comment_line<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        string("##"),
        many(satisfy(|c| c != '\n')),
        optional(newline()),
    )
        .map(|(_, content, _): (&str, Vec<char>, Option<char>)| {
            content.into_iter().collect::<String>().trim().to_string()
        })
}

fn parse_doc_comments<Input>() -> impl Parser<Input, Output = Option<String>>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    optional(many1(doc_comment_line()))
        .map(|comments: Option<Vec<String>>| comments.map(|lines| lines.join("\n")))
}

fn skip_spaces_and_comments<Input>() -> impl Parser<Input, Output = ()>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (skip_many(comment_line().skip(spaces())), spaces()).map(|_| ())
}

combine::parser! {
    fn statement_with_comments[Input]()(Input) -> Statement
    where [Input: Stream<Token = char, Position = usize>]
    {
        skip_spaces_and_comments()
            .with(statement())
            .skip(skip_spaces_and_comments())
    }
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

combine::parser! {
    fn parse_type_param[Input]()(Input) -> TypeParam
    where [Input: Stream<Token = char, Position = usize>]
    {
        (
            identifier(),
            optional(attempt(
                (
                    skip_spaces(),
                    lex_char(':'),
                    sep_by1(parse_type(), lex_char('+')),
                )
                    .map(|(_, _, bounds)| bounds),
            )),
        )
            .map(|(name, bounds_opt)| TypeParam {
                name,
                bounds: bounds_opt.unwrap_or_default(),
            })
    }
}

pub fn parse_program<Input>(file_id: FileId) -> impl Parser<Input, Output = Module>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        skip_spaces_and_comments().with((
            optional(attempt(
                choice((attempt(parse_module_binding()), parse_module_header()))
                    .skip(skip_spaces_and_comments()),
            )),
            many(
                choice((
                    attempt(parse_use()),
                    attempt(parse_module_binding()),
                    attempt(parse_wiring_site()),
                    attempt(parse_sig_definition()),
                    attempt(parse_trait_impl()),
                    attempt(parse_trait()),
                    attempt(parse_function_with_docs().map(|f| Definition::Function(Arc::new(f)))),
                    attempt(
                        parse_external_function()
                            .map(|f| Definition::ExternalFunction(Arc::new(f))),
                    ),
                    parse_struct_definition().map(|s| Definition::Struct(Arc::new(s))),
                ))
                .skip(skip_spaces_and_comments()),
            ),
        )),
        position(),
    )
        .map(move |(start, header_and_defs, end)| {
            let (header, mut definitions): (Option<Definition>, Vec<Definition>) = header_and_defs;
            if let Some(h) = header {
                definitions.insert(0, h);
            }
            Module {
                definitions,
                span: Span::new(start, end),
                file_id,
            }
        })
}

fn parse_wiring_site<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("mod"),
        identifier(),
        between(
            lex_char('('),
            lex_char(')'),
            sep_by1(identifier(), lex_char(',')),
        ),
        position(),
    )
        .map(|(start, _, name, args, end)| Definition::WiringSite {
            name,
            args,
            span: Span::new(start, end),
        })
}

fn parse_module_binding<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("mod"),
        identifier(),
        lex_char(':'),
        sep_by1(identifier_raw(), attempt(string("::"))).skip(skip_spaces()),
        lex_char('='),
        sep_by1(identifier_raw(), attempt(string("::"))).skip(skip_spaces()),
        position(),
    )
        .map(
            |(start, _, name, _, mut sig_path_vec, _, impl_path_vec, end): (
                _,
                _,
                String,
                _,
                Vec<String>,
                _,
                Vec<String>,
                _,
            )| {
                let sig_name = sig_path_vec
                    .pop()
                    .expect("sig_path requires module::SigName");
                let sig_path = NonEmpty::from_vec(sig_path_vec)
                    .expect("sig_path must have at least one module segment");
                let impl_path =
                    NonEmpty::from_vec(impl_path_vec).expect("impl_path must be non-empty");
                Definition::ModuleBinding {
                    name,
                    sig_path,
                    sig_name,
                    impl_path,
                    span: Span::new(start, end),
                }
            },
        )
}

fn parse_module_header<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("mod"),
        identifier(),
        optional(attempt(between(
            lex_char('('),
            lex_char(')'),
            sep_by(parse_module_param(), lex_char(',')),
        ))),
        position(),
    )
        .map(|(start, _, name, params, end)| Definition::ModuleHeader {
            name,
            params: params.unwrap_or_default(),
            span: Span::new(start, end),
        })
}

fn parse_module_param<Input>() -> impl Parser<Input, Output = ModuleParam>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        identifier(),
        lex_char(':'),
        sep_by1(identifier_raw(), attempt(string("::"))).skip(skip_spaces()),
        position(),
    )
        .map(|(start, name, _, path, end)| ModuleParam {
            name,
            path,
            span: Span::new(start, end),
        })
}

fn parse_sig_definition<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("sig"),
        identifier(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(
                skip_spaces_and_comments()
                    .with(parse_sig_function())
                    .skip(skip_spaces_and_comments()),
            ),
        ),
        position(),
    )
        .map(|(start, _, name, functions, end)| {
            Definition::Signature(Arc::new(AstSignature {
                name,
                functions,
                span: Span::new(start, end),
            }))
        })
}

fn parse_sig_function<Input>() -> impl Parser<Input, Output = SigFunction>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("fn"),
        identifier(),
        optional(attempt(between(
            lex_char('<'),
            lex_char('>'),
            sep_by1(parse_type_param(), lex_char(',')),
        ))),
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
            |(start, _, name, type_params_opt, parameters, _, return_type, end): (
                _,
                _,
                _,
                Option<Vec<TypeParam>>,
                _,
                _,
                _,
                _,
            )| SigFunction {
                name,
                type_params: type_params_opt.unwrap_or_default(),
                parameters,
                return_type,
                span: Span::new(start, end),
            },
        )
}

fn parse_external_function<Input>() -> impl Parser<Input, Output = ExternalFunction>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        optional(attempt(lex_string("pub"))),
        lex_string("extern"),
        lex_string("fn"),
        identifier(),
        optional(attempt(between(
            lex_char('<'),
            lex_char('>'),
            sep_by1(parse_type_param(), lex_char(',')),
        ))),
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
            |(start, pub_kw, _, _, name, type_params_opt, params, _, return_type, end): (
                _,
                _,
                _,
                _,
                _,
                Option<Vec<TypeParam>>,
                _,
                _,
                _,
                _,
            )| {
                ExternalFunction {
                    name,
                    type_params: type_params_opt.unwrap_or_default(),
                    parameters: params,
                    return_type,
                    is_pub: pub_kw.is_some(),
                    span: Span::new(start, end),
                }
            },
        )
}

fn parse_function_with_docs<Input>() -> impl Parser<Input, Output = Function>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (parse_doc_comments(), parse_function()).map(|(doc, mut func)| {
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
        optional(attempt(lex_string("pub"))),
        lex_string("fn"),
        identifier(),
        optional(attempt(between(
            lex_char('<'),
            lex_char('>'),
            sep_by1(parse_type_param(), lex_char(',')),
        ))),
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
            |(start, pub_kw, _, name, type_params_opt, params, _, return_type, body, end): (
                _,
                _,
                _,
                _,
                Option<Vec<TypeParam>>,
                _,
                _,
                _,
                _,
                _,
            )| {
                Function {
                    name,
                    type_params: type_params_opt.unwrap_or_default(),
                    parameters: params,
                    return_type,
                    body,
                    documentation: None,
                    is_pub: pub_kw.is_some(),
                    span: Span::new(start, end),
                }
            },
        )
}

fn parse_trait<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("trait"),
        identifier(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(
                skip_spaces_and_comments()
                    .with(parse_sig_function())
                    .skip(skip_spaces_and_comments()),
            ),
        ),
        position(),
    )
        .map(|(start, _, name, functions, end)| {
            Definition::Trait(Arc::new(AstTrait {
                name,
                functions,
                span: Span::new(start, end),
            }))
        })
}

fn parse_trait_impl<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("impl"),
        identifier(),
        lex_char(':'),
        identifier(),
        between(
            lex_char('{'),
            lex_char('}'),
            many(
                skip_spaces_and_comments()
                    .with(parse_function_with_docs())
                    .skip(skip_spaces_and_comments()),
            ),
        ),
        position(),
    )
        .map(|(start, _, type_name, _, trait_name, functions, end)| {
            let functions: Vec<Function> = functions;
            Definition::TraitImpl(Arc::new(AstTraitImpl {
                type_name,
                trait_name,
                functions: functions.into_iter().map(Arc::new).collect(),
                span: Span::new(start, end),
            }))
        })
}

fn parse_use<Input>() -> impl Parser<Input, Output = Definition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        optional(attempt(lex_string("pub"))),
        lex_string("use"),
        identifier_raw(),
        many1::<Vec<String>, _, _>(attempt((string("::"), identifier_raw()).map(|(_, id)| id))),
        optional(attempt(
            (skip_spaces(), lex_string("as"), identifier_raw()).map(|(_, _, a)| a),
        )),
        position(),
    )
        .map(|(start, pub_kw, _, first_seg, mut rest, alias, end)| {
            let name = rest.pop().unwrap();
            let mut path_vec = vec![first_seg];
            path_vec.extend(rest);
            let path = NonEmpty::from_vec(path_vec).unwrap();
            Definition::Use {
                path,
                name,
                alias,
                is_pub: pub_kw.is_some(),
                span: Span::new(start, end),
            }
        })
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

combine::parser! {
    fn parse_type[Input]()(Input) -> Type
    where [Input: Stream<Token = char, Position = usize>]
    {
        choice((
            attempt(
                (
                    satisfy(|c: char| c.is_uppercase()),
                    many::<Vec<char>, _, _>(combine::parser::char::alpha_num()),
                    lex_char('<'),
                    sep_by1(parse_type(), lex_char(',')),
                    lex_char('>'),
                )
                    .map(|(first, rest, _, args, _)| {
                        let name: String = std::iter::once(first).chain(rest).collect();
                        Type { name, args }
                    }),
            ),
            attempt(lex_string("()").map(|_| Type::simple("Unit"))),

            attempt(
                (
                    satisfy(|c: char| c.is_uppercase()),
                    many(combine::parser::char::alpha_num()),
                )
                    .skip(skip_spaces())
                    .map(|(first, rest): (char, Vec<char>)| {
                        Type { name: std::iter::once(first).chain(rest).collect(), args: vec![] }
                    }),
            ),
        ))
    }
}

fn parse_struct_definition<Input>() -> impl Parser<Input, Output = StructDefinition>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("struct"),
        identifier(),
        optional(attempt(between(
            lex_char('<'),
            lex_char('>'),
            sep_by1(parse_type_param(), lex_char(',')),
        ))),
        between(lex_char('{'), lex_char('}'), many(parse_struct_field())),
        position(),
    )
        .map(
            |(start, _, name, type_params_opt, fields, end): (
                _,
                _,
                _,
                Option<Vec<TypeParam>>,
                _,
                _,
            )| StructDefinition {
                name,
                type_params: type_params_opt.unwrap_or_default(),
                fields,
                span: Span::new(start, end),
            },
        )
}

fn parse_struct_field<Input>() -> impl Parser<Input, Output = StructField>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        skip_spaces_and_comments(),
        position(),
        identifier(),
        lex_char(':'),
        parse_type(),
        lex_char(','),
        position(),
    )
        .map(|(_, start, name, _, field_type, _, end)| StructField {
            name,
            field_type,
            span: Span::new(start, end),
        })
}

fn parse_function_body<Input>() -> impl Parser<Input, Output = FunctionBody>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (position(), many(statement_with_comments()), position()).map(|(start, statements, end)| {
        FunctionBody {
            statements,
            span: Span::new(start, end),
        }
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
        let primary = choice((
            attempt(parse_struct_literal()),
            attempt(parse_call()),
            parse_string_literal(),
            attempt(parse_list_literal()),
            attempt(parse_unit_literal()),
            attempt(parse_boolean_literal()),
            attempt(parse_integer_literal()),
            parse_variable(),
        ));

        (primary, many(attempt((char('.'), identifier_raw(), position()))))
            .skip(skip_spaces())
            .map(|(base, suffixes): (Expression, Vec<(char, String, usize)>)| {
                suffixes.into_iter().fold(base, |acc, (_, field, end)| {
                    let span_start = acc.span().start;
                    Expression::FieldAccess {
                        base: Box::new(acc),
                        field,
                        span: Span::new(span_start, end),
                    }
                })
            })
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

fn parse_struct_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        satisfy(|c: char| c.is_uppercase()),
        many::<Vec<char>, _, _>(combine::parser::char::alpha_num()),
        skip_spaces(),
        lex_char('{'),
        sep_by(parse_struct_field_assignment(), lex_char(',')),
        optional(lex_char(',')),
        char('}'),
        position(),
    )
        .skip(skip_spaces())
        .map(|(start, first, rest, _, _, fields, _, _, end)| {
            let struct_name: String = std::iter::once(first).chain(rest).collect();
            Expression::StructLiteral {
                struct_name,
                fields,
                span: Span::new(start, end),
            }
        })
}

fn parse_struct_field_assignment<Input>() -> impl Parser<Input, Output = (String, Expression)>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (identifier(), lex_char(':'), parse_simple_expression()).map(|(name, _, expr)| (name, expr))
}

fn parse_call<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        sep_by1(identifier_raw(), attempt(string("::"))),
        between(
            lex_char('('),
            char(')'),
            sep_by(parse_argument(), lex_char(',')),
        ),
        skip_spaces(),
        position(),
    )
        .map(
            |(start, parts, args, _, end): (usize, Vec<String>, Vec<Expression>, (), usize)| {
                Expression::Call {
                    function: parts.join("::"),
                    arguments: args,
                    span: Span::new(start, end),
                }
            },
        )
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

fn parse_integer_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        optional(char('-')),
        many1(satisfy(|c: char| c.is_ascii_digit())),
        position(),
    )
        .skip(skip_spaces())
        .map(
            |(start, sign, digits, end): (usize, Option<char>, Vec<char>, usize)| {
                let s: String = sign.into_iter().chain(digits).collect();
                let n = s.parse::<i64>().unwrap_or(0);
                Expression::IntLiteral {
                    value: n,
                    span: Span::new(start, end),
                }
            },
        )
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

fn parse_unit_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (position(), string("()"), position())
        .skip(skip_spaces())
        .map(|(start, _, end)| Expression::UnitLiteral {
            span: Span::new(start, end),
        })
}

fn parse_list_literal<Input>() -> impl Parser<Input, Output = Expression>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        between(
            lex_char('['),
            char(']'),
            sep_by(parse_simple_expression(), lex_char(',')),
        ),
        position(),
    )
        .skip(skip_spaces())
        .map(|(start, elements, end)| Expression::ListLiteral {
            elements,
            span: Span::new(start, end),
        })
}

fn parse_select<Input>() -> impl Parser<Input, Output = Statement>
where
    Input: Stream<Token = char, Position = usize>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    (
        position(),
        lex_string("select").with((
            lex_char('{'),
            skip_spaces_and_comments(),
            sep_by(
                parse_select_clause(),
                lex_char(',').skip(skip_spaces_and_comments()),
            ),
            skip_spaces_and_comments(),
            lex_char('}'),
        )),
        position(),
    )
        .map(|(start, (_, _, clauses, _, _), end)| {
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
        lex_string("select").with((
            lex_char('{'),
            skip_spaces_and_comments(),
            sep_by(
                parse_select_clause(),
                lex_char(',').skip(skip_spaces_and_comments()),
            ),
            skip_spaces_and_comments(),
            lex_char('}'),
        )),
        position(),
    )
        .map(|(start, (_, _, clauses, _, _), end)| {
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
            many(statement_with_comments()),
        ),
        optional(lex_string("else").skip(skip_spaces()).with(between(
            lex_char('{'),
            lex_char('}'),
            many(statement_with_comments()),
        ))),
        position(),
    )
        .map(
            |(start, _, condition, body, else_body, end)| Statement::If {
                condition,
                body,
                else_body,
                span: Span::new(start, end),
            },
        )
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
            many(statement_with_comments()),
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
fn analyze_code(context: String, code: String): String {
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
fn analyze_code(context: String, code: String): String {
    "Analyze the following code for potential bugs"!
    "Focus on edge cases and error handling"!
    code!
}

fn suggest_fix(context: String, analysis: String): String {
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
fn calculator_agent(ctx: String, request: String): String {
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
    fn test_parse_select_with_comments() {
        let input = r#"
fn calculator_agent(ctx: String, request: String): String {
    let result = select {
        add(ctx, _, _) as sum => sum,
        # subtract(ctx, _, _) as diff => diff,
        multiply(ctx, _, _) as product => product
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

        let Statement::Assignment {
            variable,
            expression,
            span: _,
        } = &func.body.statements[0]
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

        let Expression::Call { function, .. } = &first_clause.expression_to_run else {
            panic!("Expected call expression");
        };
        assert_eq!(function, "add");

        let second_clause = &select_stmt.clauses[1];
        assert_eq!(second_clause.result_variable, "product");

        let Expression::Call { function, .. } = &second_clause.expression_to_run else {
            panic!("Expected call expression");
        };
        assert_eq!(function, "multiply");
    }

    #[test]
    fn test_parse_select_with_multiple_commented_clauses() {
        let input = r#"
fn test_agent(ctx: String): String {
    let result = select {
        add(ctx, _, _) as sum => sum,
        # subtract(ctx, _, _) as diff => diff,
        # divide(ctx, _, _) as quotient => quotient,
        multiply(ctx, _, _) as product => product
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

        let Statement::Assignment {
            variable,
            expression,
            span: _,
        } = &func.body.statements[0]
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

        let second_clause = &select_stmt.clauses[1];
        assert_eq!(second_clause.result_variable, "product");
    }

    #[test]
    fn test_parse_select_with_comment_at_end() {
        let input = r#"
fn test_agent(ctx: String): String {
    let result = select {
        add(ctx, _, _) as sum => sum,
        subtract(ctx, _, _) as diff => diff
        # This is a trailing comment
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

        let Statement::Assignment {
            variable,
            expression,
            span: _,
        } = &func.body.statements[0]
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

        let second_clause = &select_stmt.clauses[1];
        assert_eq!(second_clause.result_variable, "diff");
    }

    #[test]
    fn test_parse_function_with_comments() {
        let input = r#"
## This function analyzes code for bugs
## It focuses on edge cases and error handling
fn analyze_code(context: String, code: String): String {
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
## Single line documentation
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
    fn test_parse_return_with_unit_literal() {
        let input = "return ()";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (statement, _) = statement().parse(stream).unwrap();
        match statement {
            Statement::Return(Expression::UnitLiteral { .. }) => {}
            _ => panic!("Expected return statement with unit literal"),
        }
    }

    #[test]
    fn test_parse_function_with_unit_literal_return() {
        let input = r#"
fn main(): () {
    return ()
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

        assert_eq!(func.name, "main");
        assert_eq!(func.body.statements.len(), 1);

        match &func.body.statements[0] {
            Statement::Return(Expression::UnitLiteral { .. }) => {}
            _ => panic!("Expected return statement with unit literal"),
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
    second_param: String,
    third_param: String
): String {
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
): String
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

    #[test]
    fn test_parse_if_else_statement() {
        let input = r#"
fn test_if_else_stmt(): () {
    if true { "then"! } else { "else"! }
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

        assert_eq!(func.body.statements.len(), 1);

        match &func.body.statements[0] {
            Statement::If {
                condition,
                body,
                else_body,
                ..
            } => {
                match condition {
                    Expression::BooleanLiteral { value, .. } => {
                        assert_eq!(*value, true);
                    }
                    _ => panic!("Expected boolean literal condition"),
                }

                assert_eq!(body.len(), 1);
                match &body[0] {
                    Statement::Injection(Expression::StringLiteral { value, .. }) => {
                        assert_eq!(value, "then");
                    }
                    _ => panic!("Expected injection with string literal in then branch"),
                }

                assert!(else_body.is_some());
                let else_body = else_body.as_ref().unwrap();
                assert_eq!(else_body.len(), 1);
                match &else_body[0] {
                    Statement::Injection(Expression::StringLiteral { value, .. }) => {
                        assert_eq!(value, "else");
                    }
                    _ => panic!("Expected injection with string literal in else branch"),
                }
            }
            _ => panic!("Expected if statement"),
        }
    }

    #[test]
    fn test_parse_list_literal() {
        let input = r#"
            fn test(): List<String> {
                let items = ["apple", "banana", "cherry"]
                items
            }
        "#;

        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID)
            .parse(stream)
            .map(|(module, _)| module);

        assert!(result.is_ok());
        let module = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        if let Definition::Function(func) = &module.definitions[0] {
            assert_eq!(func.name, "test");
            assert_eq!(func.body.statements.len(), 2);

            if let Statement::Assignment { expression, .. } = &func.body.statements[0] {
                if let Expression::ListLiteral { elements, .. } = expression {
                    assert_eq!(elements.len(), 3);
                    if let Expression::StringLiteral { value, .. } = &elements[0] {
                        assert_eq!(value, "apple");
                    } else {
                        panic!("Expected string literal");
                    }
                } else {
                    panic!("Expected list literal");
                }
            } else {
                panic!("Expected assignment");
            }
        } else {
            panic!("Expected function definition");
        }
    }

    #[test]
    fn test_parse_empty_list_literal() {
        let input = r#"
            fn test(): List<String> {
                []
            }
        "#;

        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID)
            .parse(stream)
            .map(|(module, _)| module);

        assert!(result.is_ok());
        let module = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        if let Definition::Function(func) = &module.definitions[0] {
            assert_eq!(func.body.statements.len(), 1);
            if let Statement::ExpressionStatement(Expression::ListLiteral { elements, .. }) =
                &func.body.statements[0]
            {
                assert_eq!(elements.len(), 0);
            } else {
                panic!("Expected list literal expression");
            }
        } else {
            panic!("Expected function definition");
        }
    }

    #[test]
    fn test_parse_option_type() {
        let input = r#"
            extern fn get_first(items: List<String>): Option<String>
        "#;

        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID)
            .parse(stream)
            .map(|(module, _)| module);

        assert!(result.is_ok());
        let module = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        if let Definition::ExternalFunction(func) = &module.definitions[0] {
            assert_eq!(func.name, "get_first");
            assert_eq!(func.parameters.len(), 1);
            let param_type = &func.parameters[0].param_type;
            assert_eq!(param_type.name, "List");
            assert_eq!(param_type.args[0], Type::simple("String"));
            assert_eq!(func.return_type.name, "Option");
            assert_eq!(func.return_type.args[0], Type::simple("String"));
        } else {
            panic!("Expected external function definition");
        }
    }

    #[test]
    fn test_parse_function_body_with_comments() {
        let input = r#"
fn test_function(): () {
    # This is a comment before the first statement
    let x = "hello"
    # This is a comment between statements
    x!
    # Final comment before closing
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);

        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        if let Definition::Function(func) = &module.definitions[0] {
            assert_eq!(func.name, "test_function");
            assert_eq!(func.body.statements.len(), 2);
        } else {
            panic!("Expected function definition");
        }
    }

    #[test]
    fn test_parse_comments_in_control_flow_blocks() {
        let input = r#"
fn test_function(): () {
    if true {
        # Comment in if block
        "inside if"!
    }
    while false {
        # Comment in while block
        "inside while"!
    }
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);

        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);

        if let Definition::Function(func) = &module.definitions[0] {
            assert_eq!(func.name, "test_function");
            assert_eq!(func.body.statements.len(), 2);
        } else {
            panic!("Expected function definition");
        }
    }

    #[test]
    fn test_parse_integer_literal() {
        let input = r#"
fn test(): () {
    let x = 42
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        if let Definition::Function(func) = &module.definitions[0] {
            if let Statement::Assignment { expression, .. } = &func.body.statements[0] {
                assert!(matches!(
                    expression,
                    Expression::IntLiteral { value: 42, .. }
                ));
            } else {
                panic!("Expected assignment");
            }
        }
    }

    #[test]
    fn test_parse_negative_integer_literal() {
        let input = r#"
fn test(): () {
    let x = -7
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        if let Definition::Function(func) = &module.definitions[0] {
            if let Statement::Assignment { expression, .. } = &func.body.statements[0] {
                assert!(matches!(
                    expression,
                    Expression::IntLiteral { value: -7, .. }
                ));
            } else {
                panic!("Expected assignment");
            }
        }
    }

    #[test]
    fn test_parse_zero_integer_literal() {
        let input = r#"
fn test(): () {
    let x = 0
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        if let Definition::Function(func) = &module.definitions[0] {
            if let Statement::Assignment { expression, .. } = &func.body.statements[0] {
                assert!(matches!(
                    expression,
                    Expression::IntLiteral { value: 0, .. }
                ));
            } else {
                panic!("Expected assignment");
            }
        }
    }

    #[test]
    fn test_parse_int_type() {
        let input = r#"
extern fn count(): Int
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        if let Definition::ExternalFunction(func) = &module.definitions[0] {
            assert_eq!(func.return_type, Type::simple("Int"));
        } else {
            panic!("Expected external function");
        }
    }

    #[test]
    fn test_parse_int_parameter_type() {
        let input = r#"
extern fn add(n: Int): Int
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        if let Definition::ExternalFunction(func) = &module.definitions[0] {
            assert_eq!(func.parameters[0].param_type, Type::simple("Int"));
            assert_eq!(func.return_type, Type::simple("Int"));
        } else {
            panic!("Expected external function");
        }
    }

    #[test]
    fn test_parse_struct_definition() {
        let input = "struct Point {\n    x: Int,\n    y: Int,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.name, "Point");
            assert_eq!(s.fields.len(), 2);
            assert_eq!(s.fields[0].name, "x");
            assert_eq!(s.fields[0].field_type, Type::simple("Int"));
            assert_eq!(s.fields[1].name, "y");
            assert_eq!(s.fields[1].field_type, Type::simple("Int"));
        } else {
            panic!("Expected struct definition");
        }
    }

    #[test]
    fn test_parse_struct_with_string_fields() {
        let input = "struct Task {\n    title: String,\n    done: Boolean,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.name, "Task");
            assert_eq!(s.fields[0].field_type, Type::simple("String"));
            assert_eq!(s.fields[1].field_type, Type::simple("Boolean"));
        } else {
            panic!("Expected struct definition");
        }
    }

    #[test]
    fn test_parse_struct_type_in_extern_fn() {
        let input = "struct Point {\n    x: Int,\n    y: Int,\n}\nextern fn get_point(): Point\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 2);
        if let Definition::ExternalFunction(f) = &module.definitions[1] {
            assert_eq!(f.return_type, Type::simple("Point"));
        } else {
            panic!("Expected external function");
        }
    }

    #[test]
    fn test_parse_struct_type_as_parameter() {
        let input = "struct Point {\n    x: Int,\n    y: Int,\n}\nfn describe(p: Point): String {\n    return \"ok\"\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[1] {
            assert_eq!(f.parameters[0].param_type, Type::simple("Point"));
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_struct_literal_expression() {
        let input = "fn make(): Point {\n    return Point { x: 1, y: 2 }\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[0] {
            if let crate::ast::Statement::Return(expr) = &f.body.statements[0] {
                if let Expression::StructLiteral {
                    struct_name,
                    fields,
                    ..
                } = expr
                {
                    assert_eq!(struct_name, "Point");
                    assert_eq!(fields.len(), 2);
                    assert_eq!(fields[0].0, "x");
                    assert_eq!(fields[1].0, "y");
                } else {
                    panic!("Expected StructLiteral, got {:?}", expr);
                }
            } else {
                panic!("Expected return statement");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_field_access_expression() {
        let input = "fn get_x(p: Point): Int {\n    return p.x\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        let Definition::Function(f) = &module.definitions[0] else {
            panic!()
        };
        let crate::ast::Statement::Return(expr) = &f.body.statements[0] else {
            panic!()
        };
        let Expression::FieldAccess { base, field, .. } = expr else {
            panic!()
        };
        let Expression::Variable { name, .. } = base.as_ref() else {
            panic!()
        };
        assert_eq!(name, "p");
        assert_eq!(field, "x");
    }

    #[test]
    fn test_parse_struct_and_function_together() {
        let input = "struct Task {\n    title: String,\n}\nfn make_task(): Task {\n    return Task { title: \"hello\" }\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 2);
        assert!(matches!(module.definitions[0], Definition::Struct(_)));
        assert!(matches!(module.definitions[1], Definition::Function(_)));
    }

    #[test]
    fn test_parse_struct_return_type_on_fn() {
        let input =
            "struct Point {\n    x: Int,\n}\nfn make(): Point {\n    return Point { x: 1 }\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[1] {
            assert_eq!(f.return_type, Type::simple("Point"));
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_empty_struct() {
        let input = "struct Empty {\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.name, "Empty");
            assert_eq!(s.fields.len(), 0);
        } else {
            panic!("Expected struct definition");
        }
    }

    #[test]
    fn test_parse_struct_with_list_field() {
        let input = "struct Bag {\n    items: List<String>,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.fields[0].field_type.name, "List");
            assert_eq!(s.fields[0].field_type.args[0], Type::simple("String"));
        } else {
            panic!("Expected struct definition");
        }
    }

    #[test]
    fn test_parse_struct_with_type_param() {
        let input = "struct Pair<T> {\n    first: T,\n    second: T,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.name, "Pair");
            assert_eq!(s.type_params.len(), 1);
            assert_eq!(s.type_params[0].name, "T");
            assert_eq!(s.fields.len(), 2);
        } else {
            panic!("expected struct definition");
        }
    }

    #[test]
    fn test_parse_struct_with_multiple_type_params() {
        let input = "struct Either<A, B> {\n    left: A,\n    right: B,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.type_params.len(), 2);
            assert_eq!(s.type_params[0].name, "A");
            assert_eq!(s.type_params[1].name, "B");
        } else {
            panic!("expected struct definition");
        }
    }

    #[test]
    fn test_parse_struct_without_type_params_has_empty_list() {
        let input = "struct Point {\n    x: Int,\n    y: Int,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Struct(s) = &module.definitions[0] {
            assert!(s.type_params.is_empty());
        } else {
            panic!("expected struct definition");
        }
    }

    #[test]
    fn test_parse_chained_field_access() {
        let input = "fn get_city(p: Person): String {\n    return p.address.city\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let Definition::Function(f) = &module.definitions[0] else {
            panic!()
        };
        let crate::ast::Statement::Return(expr) = &f.body.statements[0] else {
            panic!()
        };
        let Expression::FieldAccess { base, field, .. } = expr else {
            panic!()
        };
        assert_eq!(field, "city");
        let Expression::FieldAccess {
            base: inner_base,
            field: inner_field,
            ..
        } = base.as_ref()
        else {
            panic!()
        };
        assert_eq!(inner_field, "address");
        let Expression::Variable { name, .. } = inner_base.as_ref() else {
            panic!()
        };
        assert_eq!(name, "p");
    }

    #[test]
    fn test_parse_field_access_on_call() {
        let input = "fn test(): Int {\n    return make_point().x\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let Definition::Function(f) = &module.definitions[0] else {
            panic!()
        };
        let crate::ast::Statement::Return(expr) = &f.body.statements[0] else {
            panic!()
        };
        let Expression::FieldAccess { base, field, .. } = expr else {
            panic!()
        };
        assert_eq!(field, "x");
        let Expression::Call { function, .. } = base.as_ref() else {
            panic!()
        };
        assert_eq!(function, "make_point");
    }

    #[test]
    fn test_struct_literal_span_ends_at_closing_brace() {
        let input = "fn make(): Point {\n    return Point { x: 1 }\n    return p.x\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let Definition::Function(f) = &module.definitions[0] else {
            panic!()
        };
        let crate::ast::Statement::Return(expr) = &f.body.statements[0] else {
            panic!()
        };
        let Expression::StructLiteral { span, .. } = expr else {
            panic!("Expected StructLiteral")
        };
        let closing_brace_pos = input.find('}').unwrap();
        assert_eq!(
            span.end,
            closing_brace_pos + 1,
            "span.end should point just past the closing brace, not into subsequent lines"
        );
    }

    #[test]
    fn test_parse_pub_function() {
        let input = r#"
pub fn greet(name: String): String {
    return name
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function"),
        };
        assert_eq!(func.name, "greet");
        assert!(func.is_pub);
    }

    #[test]
    fn test_parse_non_pub_function_defaults_to_false() {
        let input = r#"
fn greet(name: String): String {
    return name
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function"),
        };
        assert!(!func.is_pub);
    }

    #[test]
    fn test_parse_pub_extern_function() {
        let input = "pub extern fn add(x: String, y: String): String\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        let ext = match &module.definitions[0] {
            Definition::ExternalFunction(f) => f,
            _ => panic!("Expected external function"),
        };
        assert_eq!(ext.name, "add");
        assert!(ext.is_pub);
    }

    #[test]
    fn test_parse_non_pub_extern_function_defaults_to_false() {
        let input = "extern fn add(x: String, y: String): String\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let ext = match &module.definitions[0] {
            Definition::ExternalFunction(f) => f,
            _ => panic!("Expected external function"),
        };
        assert!(!ext.is_pub);
    }

    #[test]
    fn test_parse_pub_display() {
        let input = r#"
pub fn greet(name: String): String {
    return name
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let func = match &module.definitions[0] {
            Definition::Function(f) => f,
            _ => panic!("Expected function"),
        };
        let display = format!("{}", func);
        assert!(display.starts_with("pub fn"));
    }

    #[test]
    fn test_parse_use_statement() {
        let input = "use foo::bar::baz\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        let use_def = match &module.definitions[0] {
            Definition::Use {
                path,
                name,
                alias,
                is_pub,
                ..
            } => (path.clone(), name.clone(), alias.clone(), *is_pub),
            _ => panic!("Expected Use definition"),
        };
        assert_eq!(
            use_def.0.iter().cloned().collect::<Vec<_>>(),
            vec!["foo", "bar"]
        );
        assert_eq!(use_def.1, "baz");
        assert_eq!(use_def.2, None);
        assert!(!use_def.3);
    }

    #[test]
    fn test_parse_use_with_alias() {
        let input = "use foo::bar as fb\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let use_def = match &module.definitions[0] {
            Definition::Use {
                path, name, alias, ..
            } => (path.clone(), name.clone(), alias.clone()),
            _ => panic!("Expected Use definition"),
        };
        assert_eq!(use_def.0.iter().cloned().collect::<Vec<_>>(), vec!["foo"]);
        assert_eq!(use_def.1, "bar");
        assert_eq!(use_def.2, Some("fb".to_string()));
    }

    #[test]
    fn test_parse_pub_use() {
        let input = "pub use foo::bar\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let (module, _) = parse_program(TEST_FILE_ID).parse(stream).unwrap();
        let is_pub = match &module.definitions[0] {
            Definition::Use { is_pub, .. } => *is_pub,
            _ => panic!("Expected Use definition"),
        };
        assert!(is_pub);
    }

    #[test]
    fn test_use_alias_resolves_in_typecheck() {
        let input = r#"
extern fn greet(name: String): String
use test::greet as hello
fn main(): String {
    return hello("world")
}
"#;
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok());
        let (module, _) = result.unwrap();
        use crate::typecheck::TypeChecker;
        use nonempty::NonEmpty;
        use std::collections::HashMap;
        let parsed = crate::ast::ParsedModule {
            name: NonEmpty::new("test".to_string()),
            module,
            is_entry: true,
            file_id: TEST_FILE_ID,
        };
        let result = TypeChecker::new().check(&[parsed], &HashMap::new());
        assert!(result.is_ok(), "use alias should resolve");
    }

    #[test]
    fn test_parse_struct_with_option_field() {
        let input = "struct Wrapper {\n    value: Option<Int>,\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Struct(s) = &module.definitions[0] {
            assert_eq!(s.fields[0].field_type.name, "Option");
            assert_eq!(s.fields[0].field_type.args[0], Type::simple("Int"));
        } else {
            panic!("Expected struct definition");
        }
    }

    #[test]
    fn test_parse_module_header_simple() {
        let input = "mod tasks\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 2);
        match &module.definitions[0] {
            Definition::ModuleHeader { name, params, .. } => {
                assert_eq!(name, "tasks");
                assert!(params.is_empty());
            }
            other => panic!("Expected ModuleHeader, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_module_header_with_params() {
        let input = "mod db(io: storage::Storage)\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert!(module.definitions.len() >= 2);
        match &module.definitions[0] {
            Definition::ModuleHeader { name, params, .. } => {
                assert_eq!(name, "db");
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "io");
                assert_eq!(
                    params[0].path,
                    NonEmpty::from_vec(vec![
                        "".to_string(),
                        "storage".to_string(),
                        "Storage".to_string()
                    ])
                    .unwrap()
                );
            }
            other => panic!("Expected ModuleHeader, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sig_definition_single_fn() {
        let input = "sig Greeter {\n    fn greet(name: String): String\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        match &module.definitions[0] {
            Definition::Signature(s) => {
                assert_eq!(s.name, "Greeter");
                assert_eq!(s.functions.len(), 1);
                assert_eq!(s.functions[0].name, "greet");
                assert_eq!(s.functions[0].parameters.len(), 1);
                assert_eq!(s.functions[0].parameters[0].name, "name");
                assert_eq!(s.functions[0].return_type, Type::simple("String"));
            }
            other => panic!("Expected Signature, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_sig_definition_two_fns() {
        let input = "sig Processor {\n    fn process(input: String): String\n    fn validate(input: String): Boolean\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        match &module.definitions[0] {
            Definition::Signature(s) => {
                assert_eq!(s.name, "Processor");
                assert_eq!(s.functions.len(), 2);
                assert_eq!(s.functions[0].name, "process");
                assert_eq!(s.functions[1].name, "validate");
                assert_eq!(s.functions[1].return_type, Type::simple("Boolean"));
            }
            other => panic!("Expected Signature, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_module_header_no_params_no_following_defs() {
        let input = "mod utils\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 1);
        match &module.definitions[0] {
            Definition::ModuleHeader { name, .. } => assert_eq!(name, "utils"),
            other => panic!("Expected ModuleHeader, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_module_binding() {
        let input = "mod fmt: formatter::Formatter = formatter\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 2);
        match &module.definitions[0] {
            Definition::ModuleBinding {
                name,
                sig_path,
                sig_name,
                impl_path,
                ..
            } => {
                assert_eq!(name, "fmt");
                assert_eq!(*sig_path, nonempty::nonempty!["formatter".to_string()]);
                assert_eq!(sig_name, "Formatter");
                assert_eq!(*impl_path, nonempty::nonempty!["formatter".to_string()]);
            }
            other => panic!("Expected ModuleBinding, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_module_binding_multi_segment_impl() {
        let input = "mod io: storage::Storage = storage::disk\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        match &module.definitions[0] {
            Definition::ModuleBinding {
                name,
                sig_path,
                sig_name,
                impl_path,
                ..
            } => {
                assert_eq!(name, "io");
                assert_eq!(*sig_path, nonempty::nonempty!["storage".to_string()]);
                assert_eq!(sig_name, "Storage");
                assert_eq!(
                    *impl_path,
                    nonempty::nonempty!["storage".to_string(), "disk".to_string()]
                );
            }
            other => panic!("Expected ModuleBinding, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_wiring_site() {
        let input =
            "mod fmt: formatter::Formatter = formatter\nmod reporter(fmt)\n\nfn main(): () {}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 3);
        match &module.definitions[1] {
            Definition::WiringSite { name, args, .. } => {
                assert_eq!(name, "reporter");
                assert_eq!(args, &vec!["fmt"]);
            }
            other => panic!("Expected WiringSite, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_function_with_single_type_param() {
        let input = "fn identity<T>(x: T): T {\n    return x\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[0] {
            assert_eq!(f.type_params, vec!["T"]);
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_function_with_multiple_type_params() {
        let input = "fn pair<T, U>(a: T, b: U): T {\n    return a\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[0] {
            assert_eq!(f.type_params, vec!["T", "U"]);
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_function_without_type_params_has_empty_list() {
        let input = "fn greet(name: String): String {\n    return name\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[0] {
            assert!(f.type_params.is_empty());
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_parse_sig_function_with_single_type_param() {
        let input = "sig Container {\n    fn wrap<T>(value: T): T\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Signature(s) = &module.definitions[0] {
            assert_eq!(s.functions[0].type_params, vec!["T"]);
        } else {
            panic!("Expected signature");
        }
    }

    #[test]
    fn test_parse_sig_function_with_multiple_type_params() {
        let input = "sig Mapper {\n    fn map<A, B>(input: A): B\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Signature(s) = &module.definitions[0] {
            assert_eq!(s.functions[0].type_params, vec!["A", "B"]);
        } else {
            panic!("Expected signature");
        }
    }

    #[test]
    fn test_parse_bounded_type_param() {
        let input = "fn foo<T: Add>(x: T): T {\n    return x\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[0] {
            assert_eq!(f.type_params.len(), 1);
            assert_eq!(f.type_params[0].name, "T");
            assert_eq!(f.type_params[0].bounds, vec![Type::simple("Add")]);
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_parse_multiple_bounds() {
        let input = "fn foo<T: Add + Sub>(x: T): T {\n    return x\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Function(f) = &module.definitions[0] {
            assert_eq!(
                f.type_params[0].bounds,
                vec![Type::simple("Add"), Type::simple("Sub")]
            );
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn test_parse_trait_declaration() {
        let input = "trait Add {\n    fn add(self: Self, other: Self): Self\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        if let Definition::Trait(s) = &module.definitions[0] {
            assert_eq!(s.name, "Add");
            assert_eq!(s.functions.len(), 1);
            assert_eq!(s.functions[0].name, "add");
        } else {
            panic!("expected trait, got {:?}", module.definitions[0]);
        }
    }

    #[test]
    fn test_parse_trait_impl() {
        let input = "struct Foo {\n    x: Int,\n}\nimpl Foo: Add {\n    fn add(self: Foo, other: Foo): Foo {\n        return self\n    }\n}\n";
        let stream = Stream::with_positioner(input, IndexPositioner::default());
        let result = parse_program(TEST_FILE_ID).parse(stream);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());
        let (module, _) = result.unwrap();
        assert_eq!(module.definitions.len(), 2);
        if let Definition::TraitImpl(t) = &module.definitions[1] {
            let type_name = &t.type_name;
            let trait_name = &t.trait_name;
            let functions = &t.functions;
            assert_eq!(type_name, "Foo");
            assert_eq!(trait_name, "Add");
            assert_eq!(functions.len(), 1);
            assert_eq!(functions[0].name, "add");
        } else {
            panic!("expected trait impl, got {:?}", module.definitions[1]);
        }
    }
}
