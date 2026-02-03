use std::alloc::{Allocator, Global};

use crate::{
    compiler::{asm::Label, error::Context, tacky, ty::Ty},
    intern::Interned,
};

pub use convert::Converter;

mod convert;

pub enum Item<'src, 'ty, A: Allocator = Global> {
    Fn {
        name: Interned<'src, str>,
        body: Vec<Stmt<'src, 'ty, A>>,
    },
}

pub struct Program<'src, 'ty, A: Allocator = Global> {
    pub(crate) functions: Vec<Function<'src, 'ty, A>>,
    pub(crate) globals: Vec<GlobalVar<'src>>,
}

pub struct GlobalVar<'src> {
    pub(crate) name: Interned<'src, str>,
}

pub struct Function<'src, 'ty, A: Allocator = Global> {
    pub(crate) name: Interned<'src, str>,
    pub(crate) body: Vec<Stmt<'src, 'ty, A>>,
    pub(crate) local_count: usize,
}

pub enum Stmt<'src, 'ty, A: Allocator = Global> {
    Block(Vec<Stmt<'src, 'ty, A>>),
    Decl(
        Interned<'src, str>,
        Interned<'ty, Ty<'src, 'ty>>,
        Option<Box<Expr<'src, A>, A>>,
    ),
    Expr(Box<Expr<'src, A>, A>),
    Nil,
    Return(Box<Expr<'src, A>, A>),
}

pub struct ExprWithCtx<'src, A: Allocator>(Expr<'src, A>, Context);

pub enum Expr<'src, A: Allocator> {
    Assign(AssignOp, Box<Expr<'src, A>, A>, Box<Expr<'src, A>, A>), // Op, lhs, rhs
    Binary(BinaryOp, Box<Expr<'src, A>, A>, Box<Expr<'src, A>, A>), // Op, lhs, rhs
    Unary(UnaryOp, Box<Expr<'src, A>, A>),                          // Op, operand
    DecInc(UnaryOp, Box<Expr<'src, A>, A>),                         // Op, operand (op is either ++ or --)
    Global(Interned<'src, str>),
    Var(Interned<'src, str>),
    Local(usize),
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
