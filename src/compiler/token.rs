use std::ops::Range;

use crate::compiler::error::SyntaxError;

#[derive(Clone, Debug)]
pub struct Token {
    pub(crate) ty: TokenTy,
    pub(crate) range: Range<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenTy {
    Identifier,
    Const,
    // Keywords
    Int,
    Void,
    Return,
    // Single character
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    Semicolon,
    Err(SyntaxError),
    Eof,
}
