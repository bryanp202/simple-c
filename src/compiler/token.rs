use std::ops::Range;

use crate::compiler::{
    ast::BinaryOp,
    error::{Context, SyntaxError},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenTy {
    Const,
    Identifier,
    // Keywords
    Int,
    Return,
    Void,
    // Single character
    CloseBrace,
    CloseParen,
    OpenBrace,
    OpenParen,
    Semicolon,
    Tilde,
    // Single or double
    Minus,
    MinusMinus,
    Percent,
    Plus,
    Slash,
    Star,
    // Special
    Err(SyntaxError),
    Eof,
}

impl TokenTy {
    /// The binary precedence of this tokenty and its binary op
    ///
    /// Returns none if not binary operator
    pub fn binary_prec(self) -> Option<(usize, BinaryOp)> {
        use TokenTy::*;
        match self {
            Star => Some((50, BinaryOp::Mul)),
            Slash => Some((50, BinaryOp::Div)),
            Percent => Some((50, BinaryOp::Rem)),
            Plus => Some((45, BinaryOp::Add)),
            Minus => Some((45, BinaryOp::Sub)),
            _ => None,
        }
    }
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
