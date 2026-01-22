; Keywords
"fn" @keyword
"extern" @keyword
"let" @keyword
"if" @keyword
"while" @keyword
"return" @keyword
"select" @keyword
"as" @keyword

; Built-in types
"String" @type.builtin
"Boolean" @type.builtin
"i32" @type.builtin
"Context" @type.builtin

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
  name: (identifier) @variable.parameter
  type: (type) @type)

; String literals
(string_literal) @string

; Escape sequences in strings
(escape_sequence) @string.escape

; Boolean literals
"true" @constant.builtin.boolean
"false" @constant.builtin.boolean

; Placeholder
(placeholder) @variable.special

; Comments
(comment) @comment

; Operators
"=" @operator
"+" @operator
"-" @operator
"*" @operator
"/" @operator
"==" @operator
"!=" @operator
"<" @operator
">" @operator
"<=" @operator
">=" @operator
"!" @operator
"=>" @operator

; Punctuation
"(" @punctuation.bracket
")" @punctuation.bracket
"{" @punctuation.bracket
"}" @punctuation.bracket
"," @punctuation.delimiter
":" @punctuation.delimiter
"->" @punctuation.delimiter
";" @punctuation.delimiter

; Variable declarations
(let_declaration
  name: (identifier) @variable)

; Variable assignments
(variable_assignment
  name: (identifier) @variable)

; Select bindings
(select_clause
  binding: (identifier) @variable)

; General identifiers (catch-all for remaining identifiers)
(identifier) @variable
