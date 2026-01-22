; Function text objects
(function_declaration
  body: (block
    "{" @_start
    "}" @_end
    (#not-eq? @_start @_end)) @function.inside) @function.around

(external_function_declaration) @function.around

; Comment text objects
(comment)+ @comment.around

; Block text objects (for general navigation)
(block) @class.inside

(if_statement
  consequence: (block) @class.inside) @class.around

(while_statement
  body: (block) @class.inside) @class.around

(select_expression) @class.around
