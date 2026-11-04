; TOML highlighting. Capture names are syntax token keys of the theme.
; Where captures overlap, the pattern written earlier wins.

(comment) @comment

; Table names ([package], [[bin]]) and keys.

(table
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @type)
(table_array_element
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @type)

(pair
  [
    (bare_key)
    (dotted_key)
    (quoted_key)
  ] @attribute)

; Values

(string) @string

[
  (integer)
  (float)
  (offset_date_time)
  (local_date_time)
  (local_date)
  (local_time)
] @number

(boolean) @constant
