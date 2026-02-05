use std::alloc::{Allocator, Global};

use crate::{
    compiler::{
        ast::{AssignOp, BinaryOp, UnaryOp},
        ty::Ty,
    },
    intern::Interned,
};

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
    pub(crate) local_count: usize,
}

pub enum Stmt<'src, 'a, A: Allocator = Global> {
    Block(Vec<Stmt<'src, 'a, A>>),
    Decl(Option<Box<Expr<'src, 'a, A>, A>>),
    Expr(Box<Expr<'src, 'a, A>, A>),
    If(
        Box<Expr<'src, 'a, A>, A>,
        Box<Stmt<'src, 'a, A>, A>,
        Option<Box<Stmt<'src, 'a, A>, A>>,
    ),
    Nil,
    Return(Box<Expr<'src, 'a, A>, A>),
}

pub struct Expr<'src, 'a, A: Allocator> {
    pub(crate) expr: ExprTy<'src, 'a, A>,
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
}

pub enum ExprTy<'src, 'a, A: Allocator> {
    Ternary(
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
    ),
    Assign(
        AssignOp,
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
    ), // Op, lhs, rhs
    Binary(
        BinaryOp,
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
    ), // Op, lhs, rhs
    Unary(UnaryOp, Box<Expr<'src, 'a, A>, A>), // Op, operand
    DecInc(UnaryOp, Box<Expr<'src, 'a, A>, A>), // Op, operand (op is either ++ or --)
    Global(Interned<'src, str>),
    Local(usize),
    Constant(i32),
    Poisoned,
}
