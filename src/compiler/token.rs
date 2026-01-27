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
    MinusEqual,
    Percent,
    PercentEqual,
    Plus,
    PlusPlus,
    PlusEqual,
    Slash,
    SlashEqual,
    Star,
    StarEqual,
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Greater,
    GreaterEqual,
    GreaterGreater,
    GreaterGreaterEqual,
    Less,
    LessEqual,
    LessLess,
    LessLessEqual,
    Ampersand,
    AmpersandAmpersand,
    AmpersandEqual,
    Caret,
    CaretEqual,
    Pipe,
    PipePipe,
    PipeEqual,
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
        const RELATIONAL: usize = 35;
        const EQUALITY: usize = 30;
        const BIT_AND: usize = 25;
        const BIT_XOR: usize = 20;
        const BIT_OR: usize = 15;
        const LOGIC_AND: usize = 10;
        const LOGIC_OR: usize = 5;

        match self {
            // * / %
            Star => Some((PRODUCT, BinaryOp::Mul)),
            Slash => Some((PRODUCT, BinaryOp::Div)),
            Percent => Some((PRODUCT, BinaryOp::Rem)),
            // + -
            Plus => Some((TERM, BinaryOp::Add)),
            Minus => Some((TERM, BinaryOp::Sub)),
            // >> <<
            GreaterGreater => Some((SHIFT, BinaryOp::Shr)),
            LessLess => Some((SHIFT, BinaryOp::Shl)),
            // > >= < <=
            Greater => Some((RELATIONAL, BinaryOp::G)),
            GreaterEqual => Some((RELATIONAL, BinaryOp::GE)),
            Less => Some((RELATIONAL, BinaryOp::L)),
            LessEqual => Some((RELATIONAL, BinaryOp::LE)),
            // == !=
            EqualEqual => Some((EQUALITY, BinaryOp::E)),
            BangEqual => Some((EQUALITY, BinaryOp::NE)),
            // & ^ |
            Ampersand => Some((BIT_AND, BinaryOp::BitAnd)),
            Caret => Some((BIT_XOR, BinaryOp::BitXor)),
            Pipe => Some((BIT_OR, BinaryOp::BitOr)),
            // &&
            AmpersandAmpersand => Some((LOGIC_AND, BinaryOp::And)),
            // ||
            PipePipe => Some((LOGIC_OR, BinaryOp::Or)),
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
