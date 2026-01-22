; Indent blocks
(block
  "{" @start
  "}" @end) @indent

; Indent function bodies
(function_declaration
  body: (block) @indent)

; Indent if statement bodies
(if_statement
  consequence: (block) @indent)

; Indent while statement bodies
(while_statement
  body: (block) @indent)

; Indent select expression
(select_expression
  "{" @start
  "}" @end) @indent

; Indent parameter lists
(parameter_list
  "(" @start
  ")" @end) @indent.align

; Indent argument lists
(argument_list
  "(" @start
  ")" @end) @indent.align
