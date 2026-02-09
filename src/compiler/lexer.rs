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
            return self.emit_token(TokenTy::Err(err));
        }
        let Some(c) = self.advance() else {
            return self.emit_token(TokenTy::Eof);
        };

        let ty = match c {
            // Single
            '(' => TokenTy::OpenParen,
            ')' => TokenTy::CloseParen,
            '{' => TokenTy::OpenBrace,
            '}' => TokenTy::CloseBrace,
            '[' => TokenTy::OpenSquare,
            ']' => TokenTy::CloseSquare,
            '.' => TokenTy::Dot,
            '~' => TokenTy::Tilde,
            ';' => TokenTy::Semicolon,
            ':' => TokenTy::Colon,
            '?' => TokenTy::QuestionMark,

            // Single or double
            // Arith
            '+' if self.eat_if('+') => TokenTy::PlusPlus,
            '+' if self.eat_if('=') => TokenTy::PlusEqual,
            '+' => TokenTy::Plus,
            '-' if self.eat_if('-') => TokenTy::MinusMinus,
            '-' if self.eat_if('=') => TokenTy::MinusEqual,
            '-' => TokenTy::Minus,
            '*' if self.eat_if('=') => TokenTy::StarEqual,
            '*' => TokenTy::Star,
            '/' if self.eat_if('=') => TokenTy::SlashEqual,
            '/' => TokenTy::Slash,
            '%' if self.eat_if('=') => TokenTy::PercentEqual,
            '%' => TokenTy::Percent,
            // Compare/Shift
            '>' if self.eat_if('>') => {
                if self.eat_if('=') {
                    TokenTy::GreaterGreaterEqual
                } else {
                    TokenTy::GreaterGreater
                }
            }
            '>' if self.eat_if('=') => TokenTy::GreaterEqual,
            '>' => TokenTy::Greater,
            '<' if self.eat_if('<') => {
                if self.eat_if('=') {
                    TokenTy::LessLessEqual
                } else {
                    TokenTy::LessLess
                }
            }
            '<' if self.eat_if('=') => TokenTy::LessEqual,
            '<' => TokenTy::Less,
            '=' if self.eat_if('=') => TokenTy::EqualEqual,
            '=' => TokenTy::Equal,
            '!' if self.eat_if('=') => TokenTy::BangEqual,
            '!' => TokenTy::Bang,
            // Bitwise
            '&' if self.eat_if('&') => TokenTy::AmpersandAmpersand,
            '&' if self.eat_if('=') => TokenTy::AmpersandEqual,
            '&' => TokenTy::Ampersand,
            '^' if self.eat_if('=') => TokenTy::CaretEqual,
            '^' => TokenTy::Caret,
            '|' if self.eat_if('|') => TokenTy::PipePipe,
            '|' if self.eat_if('=') => TokenTy::PipeEqual,
            '|' => TokenTy::Pipe,

            // Special
            c if c.is_ascii_digit() => self.number(),
            c if c.is_alphabetic() => self.identifier(c),
            _ => TokenTy::Err(SyntaxError::UnknownSymbol),
        };

        self.emit_token(ty)
    }
}

impl Lexer<'_> {
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
    fn number(&mut self) -> TokenTy {
        self.eat_while(|c| c.is_ascii_digit());
        match self.peek() {
            '.' | 'e' => self.float(),
            c if c.is_ascii_alphabetic() => {
                self.eat_while(|c| c.is_ascii_alphanumeric());
                TokenTy::Err(SyntaxError::InvalidIntegerSuffix)
            }
            _ => TokenTy::Const,
        }
    }

    /// A floating point literal
    fn float(&mut self) -> TokenTy {
        TokenTy::Err(SyntaxError::UnknownSymbol)
    }

    /// Check if keyword, else emit identifier
    fn identifier(&mut self, c: char) -> TokenTy {
        match c {
            'b' => return self.check_keyword(TokenTy::Break, "reak"),
            'c' => match self.peek() {
                'a' => return self.check_keyword(TokenTy::Case, "ase"),
                'o' => return self.check_keyword(TokenTy::Continue, "ontinue"),
                _ => {}
            },

            'd' => match self.peek() {
                'e' => return self.check_keyword(TokenTy::Default, "efault"),
                'o' => return self.check_keyword(TokenTy::Do, "o"),
                _ => {}
            },
            'e' => return self.check_keyword(TokenTy::Else, "lse"),
            'f' => return self.check_keyword(TokenTy::For, "or"),
            'g' => return self.check_keyword(TokenTy::Goto, "oto"),
            'i' => match self.peek() {
                'f' => return self.check_keyword(TokenTy::If, "f"),
                'n' => return self.check_keyword(TokenTy::Int, "nt"),
                _ => {}
            },

            'r' => return self.check_keyword(TokenTy::Return, "eturn"),
            's' => return self.check_keyword(TokenTy::Switch, "witch"),
            't' => return self.check_keyword(TokenTy::Typedef, "ypedef"),
            'v' => return self.check_keyword(TokenTy::Void, "oid"),
            'w' => return self.check_keyword(TokenTy::While, "hile"),
            _ => {}
        }
        self.eat_while(char::is_alphanumeric);
        TokenTy::Identifier
    }

    /// Check if the rest remaining alphanumeric chars make up the target keyword
    fn check_keyword(&mut self, target_ty: TokenTy, target: &str) -> TokenTy {
        let is_target = &self.chars.as_str()[..target.len()] == target;

        if is_target {
            self.chars = self.chars.as_str()[target.len()..].char_indices();
            target_ty
        } else {
            self.eat_while(char::is_alphanumeric);
            TokenTy::Identifier
        }
    }
}
