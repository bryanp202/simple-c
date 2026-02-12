use std::alloc::{Allocator, Global};

use crate::{
    compiler::{error::Context, ty::Ty},
    intern::Interned,
};

pub use convert::Converter;

mod convert;
mod pretty;
pub mod typed;

pub enum Item<'src, 'a, A: Allocator = Global> {
    FnDecl(FunctionDecl<'src, 'a>),
    FnDef(FunctionDef<'src, 'a, A>),
    Var(GlobalVar<'src>),
}

pub struct Program<'src, 'a, A: Allocator = Global> {
    pub(crate) items: Vec<Item<'src, 'a, A>>,
}

pub struct GlobalVar<'src> {
    pub(crate) id: Identifier<'src>,
}

pub struct FunctionDecl<'src, 'a> {
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
    pub(crate) id: Identifier<'src>,
    pub(crate) param_names: Vec<Identifier<'src>>,
}

pub struct FunctionDef<'src, 'a, A: Allocator = Global> {
    pub(crate) decl: FunctionDecl<'src, 'a>,
    pub(crate) body: Vec<Stmt<'src, 'a, A>>,
}

pub struct Identifier<'src> {
    pub(crate) name: Interned<'src, str>,
    pub(crate) ctx: Context,
}

pub struct ForStmt<'src, 'a, A: Allocator> {
    pub(crate) init: Option<Box<Stmt<'src, 'a, A>, A>>,
    pub(crate) condition: Option<Box<Expr<'src, A>, A>>,
    pub(crate) increment: Option<Box<Expr<'src, A>, A>>,
    pub(crate) body: Box<Stmt<'src, 'a, A>, A>,
}

pub enum Stmt<'src, 'a, A: Allocator = Global> {
    Block(Vec<Stmt<'src, 'a, A>>),
    Break(Context),
    Case(
        Context,
        Option<Box<Expr<'src, A>, A>>,
        Box<Stmt<'src, 'a, A>, A>,
    ),
    Continue(Context),
    Decl(
        Identifier<'src>,
        Interned<'a, Ty<'src, 'a>>,
        Option<Box<Expr<'src, A>, A>>,
    ),
    Do(Box<Stmt<'src, 'a, A>, A>, Box<Expr<'src, A>, A>),
    Expr(Box<Expr<'src, A>, A>),
    For(Box<ForStmt<'src, 'a, A>, A>),
    FunctionDecl(Box<FunctionDecl<'src, 'a>, A>),
    Goto(Identifier<'src>),
    If(
        Box<Expr<'src, A>, A>,
        Box<Stmt<'src, 'a, A>, A>,
        Option<Box<Stmt<'src, 'a, A>, A>>,
    ),
    Labled(Identifier<'src>, Box<Stmt<'src, 'a, A>, A>),
    Nil,
    Return(Box<Expr<'src, A>, A>),
    Switch(Box<Expr<'src, A>, A>, Box<Stmt<'src, 'a, A>, A>),
    While(Box<Expr<'src, A>, A>, Box<Stmt<'src, 'a, A>, A>),
}

pub struct Expr<'src, A: Allocator> {
    pub(crate) expr: ExprTy<'src, A>,
    pub(crate) ctx: Context,
}

pub struct CallExpr<'src, A: Allocator> {
    pub(crate) operand: Box<Expr<'src, A>, A>,
    pub(crate) args: Vec<Box<Expr<'src, A>, A>>,
}

pub enum ExprTy<'src, A: Allocator> {
    Ternary(
        Box<Expr<'src, A>, A>,
        Box<Expr<'src, A>, A>,
        Box<Expr<'src, A>, A>,
    ), // condition, then_branch, else_branch
    Assign(AssignOp, Box<Expr<'src, A>, A>, Box<Expr<'src, A>, A>), // Op, lhs, rhs
    Binary(BinaryOp, Box<Expr<'src, A>, A>, Box<Expr<'src, A>, A>), // Op, lhs, rhs
    Unary(UnaryOp, Box<Expr<'src, A>, A>),                          // Op, operand
    DecInc(UnaryOp, Box<Expr<'src, A>, A>), // Op, operand (op is either ++ or --)
    Call(Box<CallExpr<'src, A>, A>),
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
