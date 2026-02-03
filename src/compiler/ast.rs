use std::alloc::{Allocator, Global};

use crate::{
    compiler::{error::Context, ty::Ty},
    intern::Interned,
};

pub use convert::Converter;

mod convert;
pub mod typed;

pub enum Item<'src, 'a, A: Allocator = Global> {
    Fn {
        name: Interned<'src, str>,
        body: Vec<Stmt<'src, 'a, A>>,
    },
}

pub struct Program<'src, 'a, A: Allocator = Global> {
    pub(crate) functions: Vec<Function<'src, 'a, A>>,
    pub(crate) globals: Vec<GlobalVar<'src>>,
}

pub struct GlobalVar<'src> {
    pub(crate) name: Interned<'src, str>,
}

pub struct Function<'src, 'a, A: Allocator = Global> {
    pub(crate) name: Interned<'src, str>,
    pub(crate) body: Vec<Stmt<'src, 'a, A>>,
}

pub struct Identifier<'src> {
    pub(crate) name: Interned<'src, str>,
    pub(crate) ctx: Context,
}

pub enum Stmt<'src, 'a, A: Allocator = Global> {
    Block(Vec<Stmt<'src, 'a, A>>),
    Decl(
        Identifier<'src>,
        Interned<'a, Ty<'src, 'a>>,
        Option<Box<Expr<'src, A>, A>>,
    ),
    Expr(Box<Expr<'src, A>, A>),
    Nil,
    Return(Box<Expr<'src, A>, A>),
}

pub struct Expr<'src, A: Allocator> {
    pub(crate) expr: ExprTy<'src, A>,
    pub(crate) ctx: Context,
}

pub enum ExprTy<'src, A: Allocator> {
    Assign(AssignOp, Box<Expr<'src, A>, A>, Box<Expr<'src, A>, A>), // Op, lhs, rhs
    Binary(BinaryOp, Box<Expr<'src, A>, A>, Box<Expr<'src, A>, A>), // Op, lhs, rhs
    Unary(UnaryOp, Box<Expr<'src, A>, A>),                          // Op, operand
    DecInc(UnaryOp, Box<Expr<'src, A>, A>), // Op, operand (op is either ++ or --)
    Var(Interned<'src, str>),
    Constant(i32),
    Poisoned,
}

#[derive(Clone, Copy)]
pub enum UnaryOp {
    Increment,
    Decrement,
    Compliment,
    Negate,
    Not,
    Plus,
}

#[derive(Clone, Copy)]
pub enum BinaryOp {
    Div,
    Mul,
    Rem,
    Add,
    Sub,
    L,
    LE,
    G,
    GE,
    E,
    NE,
    Shl,
    Shr,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
}

#[derive(Clone, Copy)]
pub enum AssignOp {
    Eq,
    Div,
    Mul,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    And,
    Xor,
    Or,
}
