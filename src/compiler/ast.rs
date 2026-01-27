use std::alloc::{Allocator, Global};

use crate::{
    compiler::{asm::Label, tacky},
    intern::Interned,
};

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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Compliment,
    Negate,
    Not,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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

pub struct TackyConverter {
    temp_count: usize,
    label_count: usize,
}

impl Default for TackyConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl TackyConverter {
    pub fn new() -> Self {
        Self {
            temp_count: 0,
            label_count: 0,
        }
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

    fn new_label(&mut self) -> Label {
        let label = self.label_count;
        self.label_count += 1;
        Label(label)
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
        match ast_op {
            BinaryOp::And => {
                let lhs = self.expr(lhs, insts);
                let false_label = self.new_label();
                insts.push(tacky::Inst::JumpIfZero(lhs, false_label));
                // If !lhs, jump to false_label

                let rhs = self.expr(rhs, insts);
                insts.push(tacky::Inst::JumpIfZero(rhs, false_label));
                // If !rhs, jump to false_label

                let dst = self.new_temp();
                insts.push(tacky::Inst::Copy {
                    src: tacky::Val::Const(1),
                    dst,
                });
                let end_label = self.new_label();
                insts.push(tacky::Inst::Jump(end_label));
                // Jump to end

                // false_label:
                insts.push(tacky::Inst::Label(false_label));
                insts.push(tacky::Inst::Copy {
                    src: tacky::Val::Const(0),
                    dst,
                });
                insts.push(tacky::Inst::Label(end_label));
                // end:

                dst
            }
            BinaryOp::Or => {
                let lhs = self.expr(lhs, insts);
                let true_label = self.new_label();
                insts.push(tacky::Inst::JumpIfNotZero(lhs, true_label));
                // If lhs, jump to true_label

                let rhs = self.expr(rhs, insts);
                insts.push(tacky::Inst::JumpIfNotZero(rhs, true_label));
                // If rhs, jump to true_label

                let dst = self.new_temp();
                insts.push(tacky::Inst::Copy {
                    src: tacky::Val::Const(0),
                    dst,
                });
                let end_label = self.new_label();
                insts.push(tacky::Inst::Jump(end_label));
                // Jump to end

                // true_label:
                insts.push(tacky::Inst::Label(true_label));
                insts.push(tacky::Inst::Copy {
                    src: tacky::Val::Const(1),
                    dst,
                });
                insts.push(tacky::Inst::Label(end_label));
                // end:

                dst
            }
            _ => {
                let op = match ast_op {
                    BinaryOp::Div => tacky::BinaryOp::Div,
                    BinaryOp::Mul => tacky::BinaryOp::Mul,
                    BinaryOp::Rem => tacky::BinaryOp::Rem,
                    BinaryOp::Add => tacky::BinaryOp::Add,
                    BinaryOp::Sub => tacky::BinaryOp::Sub,
                    BinaryOp::G => tacky::BinaryOp::G,
                    BinaryOp::GE => tacky::BinaryOp::GE,
                    BinaryOp::L => tacky::BinaryOp::L,
                    BinaryOp::LE => tacky::BinaryOp::LE,
                    BinaryOp::E => tacky::BinaryOp::E,
                    BinaryOp::NE => tacky::BinaryOp::NE,
                    BinaryOp::Shl => tacky::BinaryOp::Shl,
                    BinaryOp::Shr => tacky::BinaryOp::Sar,
                    BinaryOp::BitAnd => tacky::BinaryOp::BitAnd,
                    BinaryOp::BitOr => tacky::BinaryOp::BitOr,
                    BinaryOp::BitXor => tacky::BinaryOp::BitXor,
                    _ => unreachable!(),
                };
                let lhs = self.expr(lhs, insts);
                let rhs = self.expr(rhs, insts);
                let dst = self.new_temp();

                insts.push(tacky::Inst::Binary { op, lhs, rhs, dst });
                dst
            }
        }
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
            UnaryOp::Not => tacky::UnaryOp::Not,
        };
        let src = self.expr(expr, insts);
        let dst = self.new_temp();
        insts.push(tacky::Inst::Unary { op, src, dst });
        dst
    }
}
