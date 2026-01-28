; Keywords
[
  "fn"
  "let"
  "mut"
  "const"
  "static"
  "struct"
  "enum"
  "impl"
  "trait"
  "type"
  "use"
  "pub"
  "mod"
  "if"
  "else"
  "match"
  "for"
  "while"
  "loop"
  "return"
  "break"
  "continue"
  "as"
  "async"
  "await"
  "unsafe"
  "where"
] @keyword

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Functions
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function.call)

; Macros
(macro_invocation macro: (identifier) @function.macro)

; Strings
(string_literal) @string
(char_literal) @string

; Numbers
[
  (integer_literal)
  (float_literal)
] @number

; Comments
(line_comment) @comment
(block_comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "+="
  "-="
  "*="
  "/="
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":" "."] @punctuation.delimiter
