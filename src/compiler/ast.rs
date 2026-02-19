use bitflags::bitflags;
use std::alloc::{Allocator, Global};

use crate::{
    compiler::{error::Context, ty::Ty},
    intern::Interned,
};

pub use convert::Converter;

mod convert;
mod pretty;
pub mod typed;

pub struct Program<'src, 'a, A: Allocator = Global> {
    pub(crate) items: Vec<Declaration<'src, 'a, A>>,
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SpecifierFlags: u8 {
        const Defined   = 0b0000_0001;
        const Extern    = 0b0000_0010;
        const Static    = 0b0000_0100;
    }
}

pub struct Specifiers<'src, 'a> {
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
    pub(crate) flags: SpecifierFlags,
}

pub enum Declaration<'src, 'a, A: Allocator = Global> {
    Fn(FunctionDecl<'src, 'a, A>),
    Var(VarDecl<'src, 'a, A>),
}

pub struct VarDecl<'src, 'a, A: Allocator = Global> {
    pub(crate) specifier_flags: SpecifierFlags,
    pub(crate) id: Identifier<'src>,
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
    pub(crate) init: Option<Expr<'src, A>>,
}

pub struct FunctionDecl<'src, 'a, A: Allocator = Global> {
    pub(crate) specifier_flags: SpecifierFlags,
    pub(crate) id: Identifier<'src>,
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
    pub(crate) param_names: Vec<Option<Identifier<'src>>>,
    pub(crate) body: Option<Vec<Stmt<'src, 'a, A>>>,
}

#[derive(Clone)]
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
    Decl(Box<Declaration<'src, 'a, A>, A>),
    Do(Box<Stmt<'src, 'a, A>, A>, Box<Expr<'src, A>, A>),
    Expr(Box<Expr<'src, A>, A>),
    For(Box<ForStmt<'src, 'a, A>, A>),
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
