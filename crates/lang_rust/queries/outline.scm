; Rust outline: items with their names. `@context` words are shown with
; the name (`impl Display for Cli`).

(mod_item
  name: (identifier) @name) @outline.module

(struct_item
  name: (type_identifier) @name) @outline.struct

(enum_item
  name: (type_identifier) @name) @outline.enum

(union_item
  name: (type_identifier) @name) @outline.union

(trait_item
  name: (type_identifier) @name) @outline.trait

(impl_item
  "impl" @context
  trait: (_) @name
  "for" @context
  type: (_) @name) @outline.impl

(impl_item
  "impl" @context
  !trait
  type: (_) @name) @outline.impl

(function_item
  name: (_) @name) @outline.function

(function_signature_item
  name: (_) @name) @outline.function

(const_item
  name: (identifier) @name) @outline.constant

(static_item
  name: (identifier) @name) @outline.static

(type_item
  name: (type_identifier) @name) @outline.type

(macro_definition
  name: (identifier) @name) @outline.macro
