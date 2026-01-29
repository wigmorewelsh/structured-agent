; Keywords
( "fn"
  "extern"
  "let"
  "if"
  "else"
  "while"
  "return"
  "select"
  "as") @keyword

; Built-in types recognized by the parser
( (type
    (identifier) @type.builtin)
  (#any-of? @type.builtin "String" "Boolean"))

; Unit type
(unit_type) @type

; Function declarations
(function_declaration
  name: (identifier) @function)

(external_function_declaration
  name: (identifier) @function)

; Function calls
(function_call
  function: (identifier) @function.call)

; Parameters in function declarations
(parameter
  name: (identifier) @variable.parameter)

; String literals
(string_literal) @string

; Escape sequences in strings
(escape_sequence) @string.escape

; Boolean literals
(boolean_literal) @boolean

; Placeholder
(placeholder) @constant.builtin

; Comments
(comment) @comment

; Operators
("=" "=>" "!") @operator

; Punctuation
( "("
  ")"
  "{"
  "}") @punctuation.bracket

( ","
  ":") @punctuation.delimiter

; Variable declarations
(let_declaration
  name: (identifier) @variable)

; Variable assignments
(variable_assignment
  name: (identifier) @variable)

; Select bindings
(select_clause
  binding: (identifier) @variable)

; Fallback for identifiers
(identifier) @variable
