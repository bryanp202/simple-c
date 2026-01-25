use std::str::CharIndices;

use crate::compiler::{
    error::SyntaxError,
    token::{Token, TokenTy},
};

pub const EOF_CHAR: char = '\0';

pub struct Lexer<'src> {
    len_remaining: usize,
    chars: CharIndices<'src>,
    start: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(input: &'src str) -> Self {
        Self {
            len_remaining: input.len(),
            chars: input.char_indices(),
            start: 0,
        }
    }

    pub fn advance_token(&mut self) -> Token {
        if let Err(err) = self.skip_whitespace() {
            return self.error(err);
        }
        let Some(c) = self.advance() else {
            return self.emit_token(TokenTy::Eof);
        };

        match c {
            // Single
            ';' => self.emit_token(TokenTy::Semicolon),
            '(' => self.emit_token(TokenTy::OpenParen),
            ')' => self.emit_token(TokenTy::CloseParen),
            '{' => self.emit_token(TokenTy::OpenBrace),
            '}' => self.emit_token(TokenTy::CloseBrace),
            '~' => self.emit_token(TokenTy::Tilde),

            // Single or double
            '+' => self.emit_token(TokenTy::Plus),
            '-' if self.eat_if('-') => self.emit_token(TokenTy::MinusMinus),
            '-' => self.emit_token(TokenTy::Minus),
            '*' => self.emit_token(TokenTy::Star),
            '/' => self.emit_token(TokenTy::Slash),
            '%' => self.emit_token(TokenTy::Percent),
            c if c.is_ascii_digit() => self.number(),
            c if c.is_alphabetic() => self.identifier(c),
            _ => self.error(SyntaxError::UnknownSymbol),
        }
    }
}

impl<'src> Lexer<'src> {
    #[inline]
    fn emit_token(&mut self, ty: TokenTy) -> Token {
        let new_len_remaining = self.chars.as_str().len();
        let len = self.len_remaining - new_len_remaining;
        let start = self.start;

        self.len_remaining = new_len_remaining;
        self.start += len;

        Token::new(ty, start..self.start)
    }

    #[inline]
    fn error(&mut self, err: SyntaxError) -> Token {
        self.emit_token(TokenTy::Err(err))
    }

    #[inline]
    fn advance(&mut self) -> Option<char> {
        self.chars.next().map(|(_, c)| c)
    }

    #[inline]
    fn peek(&self) -> char {
        self.chars.clone().next().map_or(EOF_CHAR, |(_, c)| c)
    }

    #[inline]
    fn peek_next(&self) -> char {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().map_or(EOF_CHAR, |(_, c)| c)
    }

    #[inline]
    fn eat_if(&mut self, expected: char) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    #[inline]
    /// Advance while a certain predicate is true
    fn eat_while(&mut self, predicate: impl Fn(char) -> bool) {
        while predicate(self.peek()) {
            self.advance();
        }
    }

    #[inline]
    /// Advance while a certain predicate is false
    fn eat_while_not(&mut self, predicate: impl Fn(char) -> bool) {
        while !predicate(self.peek()) {
            self.advance();
        }
    }

    /// Skip all whitespace while keeping track of line and column numbers
    fn skip_whitespace(&mut self) -> Result<(), SyntaxError> {
        loop {
            match self.peek() {
                '/' if self.peek_next() == '/' => self.line_comment(),
                '/' if self.peek_next() == '*' => self.block_comment()?,
                ' ' | '\t' | '\r' | '\n' => _ = self.advance(),
                _ => break,
            }
        }
        let new_len_remaining = self.chars.as_str().len();
        let skipped = self.len_remaining - new_len_remaining;
        self.len_remaining = new_len_remaining;
        self.start += skipped;
        Ok(())
    }

    /// A '//' style comment, Goes until end of line
    fn line_comment(&mut self) {
        self.advance();
        self.advance();
        while let Some(c) = self.advance() {
            if c == '\n' {
                return;
            }
        }
    }

    /// A '/* */' style comment, Goes until closer or end of file
    fn block_comment(&mut self) -> Result<(), SyntaxError> {
        self.advance();
        self.advance();

        while let Some(c) = self.advance() {
            if c == '*' && self.peek() == '/' {
                return Ok(());
            }
        }

        Err(SyntaxError::UnterminatedBlockComment)
    }

    /// A number, both integers and floats
    fn number(&mut self) -> Token {
        self.eat_while(|c| c.is_ascii_alphanumeric());
        match self.peek() {
            '.' | 'e' => self.float(),
            c if c.is_ascii_alphabetic() => {
                self.eat_while(|c| c.is_ascii_alphanumeric());
                self.error(SyntaxError::InvalidIntegerSuffix)
            }
            _ => self.emit_token(TokenTy::Const),
        }
    }

    /// A floating point literal
    fn float(&mut self) -> Token {
        self.error(SyntaxError::UnknownSymbol)
    }

    /// Check if keyword, else emit identifier
    fn identifier(&mut self, c: char) -> Token {
        match c {
            'i' => return self.check_keyword(TokenTy::Int, "nt"),
            'r' => return self.check_keyword(TokenTy::Return, "eturn"),
            'v' => return self.check_keyword(TokenTy::Void, "oid"),
            _ => {}
        }
        self.eat_while(char::is_alphanumeric);
        self.emit_token(TokenTy::Identifier)
    }

    /// Check if the rest remaining alphanumeric chars make up the target keyword
    fn check_keyword(&mut self, target_ty: TokenTy, target: &str) -> Token {
        let is_target = &self.chars.as_str()[..target.len()] == target;

        if is_target {
            self.chars = self.chars.as_str()[target.len()..].char_indices();
            self.emit_token(target_ty)
        } else {
            self.eat_while(char::is_alphanumeric);
            self.emit_token(TokenTy::Identifier)
        }
    }
}

pub fn tokenize(input: &str) -> impl Iterator<Item = Token> {
    let mut lexer = Lexer::new(input);
    std::iter::from_fn(move || {
        let token = lexer.advance_token();
        if TokenTy::Eof == token.ty {
            None
        } else {
            Some(token)
        }
    })
}
