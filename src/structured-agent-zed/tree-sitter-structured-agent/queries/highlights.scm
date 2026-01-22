; Keywords
["fn" "extern" "let" "return" "if" "while" "as" "select"] @keyword

; Operators
["=" "=>" "!"] @operator

; Punctuation
["(" ")" "{" "}" "," ":"] @punctuation.delimiter

; String literals
(string_literal) @string
(escape_sequence) @string.escape

; Comments
(comment) @comment

; Types
(type) @type

; Function definitions
(function_declaration
  name: (identifier) @function)

(external_function_declaration
  name: (identifier) @function)

; Function calls
(function_call
  function: (identifier) @function.call)

; Parameters
(parameter
  name: (identifier) @variable.parameter)

; Variable assignments and declarations
(let_declaration
  name: (identifier) @variable)

(variable_assignment
  name: (identifier) @variable)

; Select clauses
(select_clause
  binding: (identifier) @variable)

; Boolean literals
(boolean_literal) @boolean

; Placeholder
(placeholder) @constant.builtin

; All other identifiers as variables
(identifier) @variable
