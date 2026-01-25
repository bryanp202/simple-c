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
    Equal,
    EqualEqual,
    Greater,
    GreaterGreater,
    Less,
    LessLess,
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
        const PRODUCT: usize = 50;
        const TERM: usize = 45;
        const SHIFT: usize = 40;

        match self {
            Star => Some((PRODUCT, BinaryOp::Mul)),
            Slash => Some((PRODUCT, BinaryOp::Div)),
            Percent => Some((PRODUCT, BinaryOp::Rem)),
            Plus => Some((TERM, BinaryOp::Add)),
            Minus => Some((TERM, BinaryOp::Sub)),
            GreaterGreater => Some((SHIFT, BinaryOp::Shr)),
            LessLess => Some((SHIFT, BinaryOp::Shl)),
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
