
; Keywords
[
  "fn"
  "extern"
  "let"
  "if"
  "else"
  "while"
  "return"
  "select"
  "as"
] @keyword

; Punctuation
[
  "("
  ")"
  "{"
  "}"
] @punctuation.bracket

[
  ":"
  ","
] @punctuation.delimiter

; Operators
[
  "="
  "=>"
  "!"
] @operator

; Literals
(string_literal) @string
(escape_sequence) @string.escape
(boolean_literal) @constant.builtin.boolean
(placeholder) @constant.builtin

; Comments
(comment) @comment

; Types
(type) @type.builtin

; Functions
(function_declaration
  name: (identifier) @function)

(external_function_declaration
  name: (identifier) @function)

; (function_call
;   name: (identifier) @function.call)

; Variables and Parameters
(parameter
  name: (identifier) @variable.parameter)

(let_declaration
  name: (identifier) @variable)

(variable_assignment
  name: (identifier) @variable)

(select_clause
  binding: (identifier) @variable)

; Fallback for any other identifiers
(identifier) @variable
