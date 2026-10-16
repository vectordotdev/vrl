The lexer no longer panics on a program with consecutive comment lines inside a block when a
later comment contains a non-ASCII character, and no longer fails to lex such a program when a
later comment contains a quote.

authors: DeviousCardi
