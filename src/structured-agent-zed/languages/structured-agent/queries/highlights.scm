; Keywords as literal strings
"fn" @keyword
"extern" @keyword
"let" @keyword
"if" @keyword
"while" @keyword
"return" @keyword
"select" @keyword
"as" @keyword

; Type keywords as literal strings
"String" @type
"Boolean" @type
"i32" @type
"Context" @type

; Unit type
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
"true" @boolean
"false" @boolean

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

; Identifiers (variables)
(identifier) @variable

; Let bindings - higher precedence
(let_declaration
  name: (identifier) @variable)

(variable_assignment
  name: (identifier) @variable)

; Select bindings
(select_clause
  binding: (identifier) @variable)
