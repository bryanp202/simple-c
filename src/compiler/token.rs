use std::ops::Range;

use crate::compiler::error::{Context, SyntaxError};

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
    Tilde,
    // Single or double
    Minus,
    MinusMinus,
    // Special
    Err(SyntaxError),
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub(crate) ty: TokenTy,
    pub(crate) ctx: Context,
}

impl Token {
    #[inline]
    pub fn new(ty: TokenTy, ctx: Range<usize>) -> Self {
        Self {
            ty,
            ctx: Context::from(ctx),
        }
    }
}
