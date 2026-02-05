use std::ops::Range;

use crate::compiler::error::{Context, SyntaxError};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenTy {
    Const,
    Identifier,
    // Keywords
    Else,
    If,
    Int,
    Return,
    Typedef,
    Void,
    // Single character
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenSquare,
    CloseSquare,
    Semicolon,
    Tilde,
    Dot,
    QuestionMark,
    Colon,
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

#[repr(usize)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Precedence {
    None,
    Conditional,
    Assignment,
    LogicOr,
    LogicAnd,
    BitOr,
    BitXor,
    BitAnd,
    Equality,
    Relational,
    Shift,
    Term,
    Product,
    Unary,
    Postfix,
    Primary,
}

impl Precedence {
    pub const fn up(self) -> Self {
        use Precedence::*;
        match self {
            None => Conditional,
            Conditional => Assignment,
            Assignment => LogicOr,
            LogicOr => LogicAnd,
            LogicAnd => BitOr,
            BitOr => BitXor,
            BitXor => BitAnd,
            BitAnd => Equality,
            Equality => Relational,
            Relational => Shift,
            Shift => Term,
            Term => Product,
            Product => Unary,
            Unary => Postfix,
            Postfix => Primary,
            Primary => Primary,
        }
    }
}

impl TokenTy {
    pub const fn anyfix_precedence(self) -> Precedence {
        match self {
            TokenTy::PlusPlus | TokenTy::MinusMinus => Precedence::Postfix,
            TokenTy::Star | TokenTy::Slash | TokenTy::Percent => Precedence::Product,
            TokenTy::Plus | TokenTy::Minus => Precedence::Term,
            TokenTy::GreaterGreater | TokenTy::LessLess => Precedence::Shift,
            TokenTy::GreaterEqual | TokenTy::Greater | TokenTy::LessEqual | TokenTy::Less => {
                Precedence::Relational
            }
            TokenTy::EqualEqual | TokenTy::BangEqual => Precedence::Equality,
            TokenTy::Ampersand => Precedence::BitAnd,
            TokenTy::Caret => Precedence::BitXor,
            TokenTy::Pipe => Precedence::BitOr,
            TokenTy::AmpersandAmpersand => Precedence::LogicAnd,
            TokenTy::PipePipe => Precedence::LogicOr,
            TokenTy::Dot | TokenTy::OpenSquare | TokenTy::OpenParen => Precedence::Postfix,
            TokenTy::QuestionMark => Precedence::Conditional,
            TokenTy::Equal
            | TokenTy::PlusEqual
            | TokenTy::MinusEqual
            | TokenTy::StarEqual
            | TokenTy::SlashEqual
            | TokenTy::PercentEqual
            | TokenTy::LessLessEqual
            | TokenTy::GreaterGreaterEqual
            | TokenTy::AmpersandEqual
            | TokenTy::CaretEqual
            | TokenTy::PipeEqual => Precedence::Assignment,
            _ => Precedence::None,
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
