; TOML outline: tables, then their keys nested inside them.

(table
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @name) @outline.table

(table_array_element
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @name) @outline.table

(pair
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @name) @outline.key
