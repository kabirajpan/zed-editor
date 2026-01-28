; Keywords
[
  "function"
  "const"
  "let"
  "var"
  "if"
  "else"
  "for"
  "while"
  "return"
  "break"
  "continue"
  "class"
  "extends"
  "async"
  "await"
  "import"
  "export"
  "from"
  "default"
  "new"
  "typeof"
  "instanceof"
] @keyword

; Functions
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function.call)

; Strings
(string) @string
(template_string) @string

; Numbers
(number) @number

; Comments
(comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "=="
  "==="
  "!="
  "!=="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":" "."] @punctuation.delimiter
