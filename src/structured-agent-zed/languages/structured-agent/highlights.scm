; Keywords - using simple literal matching
"fn" @keyword
"let" @keyword
"return" @keyword

; String literals
(string_literal) @string

; Comments
(comment) @comment

; Function names
(function_declaration
  name: (identifier) @function)

; All other identifiers
(identifier) @variable
