; Keywords
[
  "fn"
  "extern"
  "let"
  "if"
  "while"
  "return"
  "select"
  "as"
] @keyword

; Types
[
  "String"
  "Boolean"
  "i32"
  "Context"
] @type

(unit_type) @type

; Function definitions
(function_declaration
  name: (identifier) @function)

(external_function_declaration
  name: (identifier) @function)

; Function calls
(function_call
  function: (identifier) @function)

; Parameters
(parameter
  name: (identifier) @variable.parameter)

; String literals
(string_literal) @string

; Escape sequences in strings
(escape_sequence) @string.escape

; Boolean literals
(boolean_literal) @boolean

; Placeholder
(placeholder) @variable.special

; Comments
(comment) @comment

; Operators
[
  "="
  "+"
  "-"
  "*"
  "/"
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "!"
  "=>"
] @operator

; Punctuation
[
  "("
  ")"
  "{"
  "}"
  ","
  ":"
  "->"
] @punctuation.delimiter

; Identifiers (variables)
(identifier) @variable

; Let bindings
(let_declaration
  name: (identifier) @variable)

(variable_assignment
  name: (identifier) @variable)

; Select bindings
(select_clause
  binding: (identifier) @variable)
