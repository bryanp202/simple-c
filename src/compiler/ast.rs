use std::alloc::{Allocator, Global};

use crate::{compiler::tacky, intern::Interned};

pub struct Program<'src, A: Allocator = Global> {
    pub(crate) item: Item<'src, A>,
}

pub enum Item<'src, A: Allocator = Global> {
    Fn {
        name: Interned<'src, str>,
        body: Stmt<A>,
    },
}

pub enum Stmt<A: Allocator = Global> {
    Return(Box<Expr<A>, A>),
}

#[derive(PartialEq, Eq, Hash)]
pub enum Expr<A: Allocator> {
    Constant(i32),
    Unary(UnaryOp, Box<Expr<A>, A>),                    // Op, operand
    Binary(BinaryOp, Box<Expr<A>, A>, Box<Expr<A>, A>), // Op, lhs, rhs
}

#[derive(PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Compliment,
    Negate,
}

#[derive(PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul,
    Rem,
    Shl,
    Shr,
    BitAnd,
    BitXor,
    BitOr,
}

pub struct TackyConverter {
    temp_count: usize,
}

impl Default for TackyConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl TackyConverter {
    pub fn new() -> Self {
        Self { temp_count: 0 }
    }

    pub fn convert<'src, A: Allocator>(
        &mut self,
        program: Program<'src, A>,
    ) -> tacky::Program<'src> {
        let item = self.item(program.item);
        tacky::Program { item }
    }
}

impl<'src> TackyConverter {
    fn new_temp(&mut self) -> tacky::Val<'src> {
        let temp = self.temp_count;
        self.temp_count += 1;
        tacky::Val::Temp(temp)
    }

    fn item(&mut self, item: Item<'src, impl Allocator>) -> tacky::Item<'src> {
        match item {
            Item::Fn { name, body } => {
                let mut insts = Vec::new();
                self.stmt(body, &mut insts);
                tacky::Item::Fn { name, insts }
            }
        }
    }

    fn stmt(&mut self, stmt: Stmt<impl Allocator>, insts: &mut Vec<tacky::Inst<'src>>) {
        match stmt {
            Stmt::Return(expr) => {
                let src = self.expr(*expr, insts);
                insts.push(tacky::Inst::Ret(src));
            }
        }
    }

    fn expr(
        &mut self,
        expr: Expr<impl Allocator>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        match expr {
            Expr::Constant(imm) => tacky::Val::Const(imm),
            Expr::Unary(op, expr) => self.unary(op, *expr, insts),
            Expr::Binary(op, lhs, rhs) => self.binary(op, *lhs, *rhs, insts),
        }
    }

    fn binary<A: Allocator>(
        &mut self,
        ast_op: BinaryOp,
        lhs: Expr<A>,
        rhs: Expr<A>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let op = match ast_op {
            BinaryOp::Add => tacky::BinaryOp::Add,
            BinaryOp::Div => tacky::BinaryOp::Div,
            BinaryOp::Mul => tacky::BinaryOp::Mul,
            BinaryOp::Rem => tacky::BinaryOp::Rem,
            BinaryOp::Sub => tacky::BinaryOp::Sub,
            BinaryOp::Shl => tacky::BinaryOp::Shl,
            BinaryOp::Shr => tacky::BinaryOp::Sar,
            BinaryOp::BitAnd => tacky::BinaryOp::BitAnd,
            BinaryOp::BitOr => tacky::BinaryOp::BitOr,
            BinaryOp::BitXor => tacky::BinaryOp::BitXor,
        };
        let lhs = self.expr(lhs, insts);
        let rhs = self.expr(rhs, insts);
        let dst = self.new_temp();
        insts.push(tacky::Inst::Binary { op, lhs, rhs, dst });
        dst
    }

    fn unary(
        &mut self,
        ast_op: UnaryOp,
        expr: Expr<impl Allocator>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let op = match ast_op {
            UnaryOp::Compliment => tacky::UnaryOp::Compliment,
            UnaryOp::Negate => tacky::UnaryOp::Negate,
        };
        let src = self.expr(expr, insts);
        let dst = self.new_temp();
        insts.push(tacky::Inst::Unary { op, src, dst });
        dst
    }
}
