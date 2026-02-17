(function_item
    name: (identifier) @name
    (#eq? @name "{symbol}")) @definition

(struct_item
    name: (type_identifier) @name
    (#eq? @name "{symbol}")) @definition

(enum_item
    name: (type_identifier) @name
    (#eq? @name "{symbol}")) @definition

(type_item
    name: (type_identifier) @name
    (#eq? @name "{symbol}")) @definition

(trait_item
    name: (type_identifier) @name
    (#eq? @name "{symbol}")) @definition

(impl_item
    type: (type_identifier) @name
    (#eq? @name "{symbol}")) @definition
