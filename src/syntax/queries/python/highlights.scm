; Keywords
[
  "def"
  "class"
  "if"
  "elif"
  "else"
  "for"
  "while"
  "return"
  "break"
  "continue"
  "import"
  "from"
  "as"
  "with"
  "try"
  "except"
  "finally"
  "raise"
  "async"
  "await"
  "lambda"
] @keyword

; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function.call)

; Strings
(string) @string

; Numbers
(integer) @number
(float) @number

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
  "!="
  "<"
  ">"
  "<="
  ">="
  "and"
  "or"
  "not"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "."] @punctuation.delimiter
