use std::alloc::{Allocator, Global};

use crate::{
    compiler::{
        asm::Label,
        ast::{AssignOp, BinaryOp, UnaryOp},
        ty::Ty,
    },
    intern::Interned,
};

pub struct Program<'src, 'a, A: Allocator = Global> {
    pub(crate) labels: usize,
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

pub struct ForStmt<'src, 'a, A: Allocator> {
    pub(crate) init: Option<Box<Stmt<'src, 'a, A>, A>>,
    pub(crate) condition: Option<Box<Expr<'src, 'a, A>, A>>,
    pub(crate) increment: Option<Box<Expr<'src, 'a, A>, A>>,
    pub(crate) body: Box<Stmt<'src, 'a, A>, A>,
}

pub struct SwitchStmt<'src, 'a, A: Allocator> {
    pub(crate) expr: Box<Expr<'src, 'a, A>, A>,
    pub(crate) cases: Vec<SwitchCase>,
    pub(crate) default: Option<Label>,
    pub(crate) body: Box<Stmt<'src, 'a, A>, A>,
}

pub struct SwitchCase {
    pub(crate) val: i32,
    pub(crate) label: Label,
}

pub enum Stmt<'src, 'a, A: Allocator = Global> {
    Break,
    Block(Vec<Stmt<'src, 'a, A>>),
    Continue,
    Decl(Option<Box<Expr<'src, 'a, A>, A>>),
    Do(Box<Stmt<'src, 'a, A>, A>, Box<Expr<'src, 'a, A>, A>),
    Expr(Box<Expr<'src, 'a, A>, A>),
    For(Box<ForStmt<'src, 'a, A>, A>),
    Goto(Label),
    If(
        Box<Expr<'src, 'a, A>, A>,
        Box<Stmt<'src, 'a, A>, A>,
        Option<Box<Stmt<'src, 'a, A>, A>>,
    ),
    Labled(Label, Box<Stmt<'src, 'a, A>, A>),
    Nil,
    Return(Box<Expr<'src, 'a, A>, A>),
    Switch(Box<SwitchStmt<'src, 'a, A>, A>),
    While(Box<Expr<'src, 'a, A>, A>, Box<Stmt<'src, 'a, A>, A>),
}

pub struct Expr<'src, 'a, A: Allocator> {
    pub(crate) expr: ExprTy<'src, 'a, A>,
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
}

pub struct CallExpr<'src, 'a, A: Allocator> {
    pub(crate) operand: Box<Expr<'src, 'a, A>, A>,
    pub(crate) args: Vec<Box<Expr<'src, 'a, A>, A>, A>,
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
    Call(Box<CallExpr<'src, 'a, A>, A>),
    Global(Interned<'src, str>),
    Local(usize),
    Constant(i32),
    Poisoned,
}
