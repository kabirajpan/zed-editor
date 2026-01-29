; Keywords
[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "else"
  "enum"
  "fn"
  "for"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "match"
  "mod"
  "pub"
  "return"
  "static"
  "struct"
  "trait"
  "type"
  "unsafe"
  "use"
  "where"
  "while"
] @keyword

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Functions
(function_item
  name: (identifier) @function)

(call_expression
  function: (identifier) @function.call)

; Macros
(macro_invocation
  macro: (identifier) @function.macro)

; Strings
(string_literal) @string
(char_literal) @string

; Numbers
(integer_literal) @number
(float_literal) @number

; Comments
(line_comment) @comment
(block_comment) @comment

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":"] @punctuation.delimiter
