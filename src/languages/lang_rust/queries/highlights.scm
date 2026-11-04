; Rust highlighting. Capture names are syntax token keys of the theme.
; Where captures overlap, the pattern written earlier wins: comments and
; attributes color everything inside them.

(line_comment doc: (doc_comment)) @doc_comment
(block_comment doc: (doc_comment)) @doc_comment
(line_comment) @comment
(block_comment) @comment
(shebang) @comment

(attribute_item) @attribute
(inner_attribute_item) @attribute

; Literals

[
  (string_literal)
  (raw_string_literal)
  (char_literal)
] @string

[
  (integer_literal)
  (float_literal)
] @number

(boolean_literal) @constant

(lifetime) @lifetime
(label) @lifetime

; Macros

(macro_invocation
  macro: (identifier) @macro
  "!" @macro)
(macro_invocation
  macro: (scoped_identifier
    name: (identifier) @macro)
  "!" @macro)
(macro_definition
  name: (identifier) @macro)

; Functions: definitions and calls (tuple structs and enum variants called
; like functions, such as `Ok(())`, are colored as calls).

(function_item
  name: (identifier) @function)
(function_signature_item
  name: (identifier) @function)
(call_expression
  function: (identifier) @function)
(call_expression
  function: (scoped_identifier
    name: (identifier) @function))
(call_expression
  function: (field_expression
    field: (field_identifier) @function))
(generic_function
  function: (identifier) @function)
(generic_function
  function: (scoped_identifier
    name: (identifier) @function))
(generic_function
  function: (field_expression
    field: (field_identifier) @function))

; Keywords

[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "default"
  "dyn"
  "else"
  "enum"
  "extern"
  "fn"
  "for"
  "gen"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "macro_rules!"
  "match"
  "mod"
  "move"
  "pub"
  "raw"
  "ref"
  "return"
  "static"
  "struct"
  "trait"
  "type"
  "union"
  "unsafe"
  "use"
  "where"
  "while"
  "yield"
] @keyword

[
  (crate)
  (super)
  (self)
  (mutable_specifier)
] @keyword

(metavariable) @constant

; Constants and types by naming convention: SCREAMING_CASE is a constant,
; other capitalized names are types (or enum variants, such as `None`).

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]+$"))

[
  (type_identifier)
  (primitive_type)
] @type

((identifier) @type
  (#match? @type "^[A-Z]"))
