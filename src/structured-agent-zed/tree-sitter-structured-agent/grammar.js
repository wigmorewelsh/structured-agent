module.exports = grammar({
  name: "structured_agent",

  extras: ($) => [/\s/, $.comment],

  word: ($) => $.identifier,

  rules: {
    source_file: ($) => repeat($.definition),

    definition: ($) =>
      choice($.function_declaration, $.external_function_declaration),

    comment: ($) => token(seq("#", /.*/)),

    function_declaration: ($) =>
      seq(
        optional(repeat1($.comment)),
        "fn",
        field("name", $.identifier),
        field("parameters", $.parameter_list),
        ":",
        field("return_type", $.type),
        field("body", $.block),
      ),

    external_function_declaration: ($) =>
      seq(
        optional(repeat1($.comment)),
        "extern",
        "fn",
        field("name", $.identifier),
        field("parameters", $.parameter_list),
        ":",
        field("return_type", $.type),
      ),

    parameter_list: ($) =>
      seq(
        "(",
        optional(
          seq($.parameter, repeat(seq(",", $.parameter)), optional(",")),
        ),
        ")",
      ),

    parameter: ($) =>
      seq(field("name", $.identifier), ":", field("type", $.type)),

    type: ($) =>
      choice("String", "Boolean", "i32", "Context", $.identifier, $.unit_type),

    unit_type: ($) => seq("(", ")"),

    block: ($) => seq("{", repeat($.statement), "}"),

    statement: ($) =>
      choice(
        $.injection,
        $.let_declaration,
        $.variable_assignment,
        $.if_statement,
        $.while_statement,
        $.return_statement,
        $.expression_statement,
      ),

    injection: ($) => seq(choice($.string_literal, $.identifier), "!"),

    let_declaration: ($) =>
      seq(
        "let",
        field("name", $.identifier),
        "=",
        field("value", $.expression),
      ),

    variable_assignment: ($) =>
      seq(field("name", $.identifier), "=", field("value", $.expression)),

    expression_statement: ($) => $.expression,

    if_statement: ($) =>
      seq(
        "if",
        field("condition", $.expression),
        field("consequence", $.block),
      ),

    while_statement: ($) =>
      seq("while", field("condition", $.expression), field("body", $.block)),

    return_statement: ($) => seq("return", $.expression),

    expression: ($) =>
      choice(
        $.function_call,
        $.select_expression,
        $.identifier,
        $.string_literal,
        $.boolean_literal,
        $.placeholder,
      ),

    function_call: ($) =>
      seq(field("function", $.identifier), field("arguments", $.argument_list)),

    argument_list: ($) =>
      seq(
        "(",
        optional(
          seq($.expression, repeat(seq(",", $.expression)), optional(",")),
        ),
        ")",
      ),

    select_expression: ($) => seq("select", "{", repeat1($.select_clause), "}"),

    select_clause: ($) =>
      seq(
        field("call", $.function_call),
        "as",
        field("binding", $.identifier),
        "=>",
        field("result", $.expression),
        optional(","),
      ),

    identifier: ($) => /[a-zA-Z_][a-zA-Z0-9_]*/,

    string_literal: ($) =>
      seq(
        '"',
        repeat(choice(token.immediate(prec(1, /[^"\\]+/)), $.escape_sequence)),
        '"',
      ),

    escape_sequence: ($) =>
      token.immediate(
        seq("\\", choice(/[\\'"nrt]/, /u[0-9a-fA-F]{4}/, /U[0-9a-fA-F]{8}/)),
      ),

    boolean_literal: ($) => choice("true", "false"),

    placeholder: ($) => "_",
  },
});
