use std::alloc::{Allocator, Global};

use crate::{
    compiler::{asm::Label, tacky, ty::Ty},
    intern::Interned,
};

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

pub struct TackyConverter {
    temp_count: usize,
    label_count: usize,
    curr_local: usize,
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
            curr_local: 0,
        }
    }

    pub fn convert<'src, 'ty, A: Allocator>(
        &mut self,
        program: Program<'src, 'ty, A>,
    ) -> tacky::Program<'src> {
        let Program { globals, functions } = program;
        let globals = globals
            .into_iter()
            .map(|global| self.global(global))
            .collect();
        let functions = functions
            .into_iter()
            .map(|fun| self.function(fun))
            .collect();
        tacky::Program { globals, functions }
    }
}

impl<'src, 'ty> TackyConverter {
    #[inline]
    fn reset_for_fn(&mut self, local_count: usize) {
        self.temp_count = local_count;
        self.curr_local = 0;
    }

    #[inline]
    fn new_local(&mut self) -> tacky::Val<'src> {
        let local = self.curr_local;
        self.curr_local += 1;
        tacky::Val::Temp(local)
    }

    #[inline]
    fn new_temp(&mut self) -> tacky::Val<'src> {
        let temp = self.temp_count;
        self.temp_count += 1;
        tacky::Val::Temp(temp)
    }

    #[inline]
    fn new_label(&mut self) -> Label {
        let label = self.label_count;
        self.label_count += 1;
        Label(label)
    }

    fn global(&self, global: GlobalVar<'src>) -> tacky::GlobalVar<'src> {
        let GlobalVar { name } = global;
        tacky::GlobalVar { name }
    }

    fn function(&mut self, fun: Function<'src, 'ty, impl Allocator>) -> tacky::Function<'src> {
        let Function {
            name,
            body,
            local_count,
        } = fun;
        self.reset_for_fn(local_count);
        let mut insts = Vec::new();

        for stmt in body {
            self.stmt(stmt, &mut insts);
        }
        // Add catch all null(0) return
        insts.push(tacky::Inst::Ret(tacky::Val::Const(0)));

        tacky::Function { name, insts }
    }

    fn stmt(&mut self, stmt: Stmt<'src, 'ty, impl Allocator>, insts: &mut Vec<tacky::Inst<'src>>) {
        match stmt {
            Stmt::Block(stmts) => {
                for stmt in stmts {
                    self.stmt(stmt, insts);
                }
            }
            Stmt::Decl(_, _, init) => {
                if let Some(init) = init {
                    let src = self.expr(*init, insts);
                    insts.push(tacky::Inst::Copy {
                        src,
                        dst: self.new_local(),
                    });
                }
            }
            Stmt::Expr(expr) => _ = self.expr(*expr, insts),
            Stmt::Nil => {} // Do nothing
            Stmt::Return(expr) => {
                let src = self.expr(*expr, insts);
                insts.push(tacky::Inst::Ret(src));
            }
        }
    }

    fn expr(
        &mut self,
        expr: Expr<'src, impl Allocator>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        match expr {
            Expr::Assign(op, lhs, rhs) => self.assign(op, *lhs, *rhs, insts),
            Expr::Binary(op, lhs, rhs) => self.binary(op, *lhs, *rhs, insts),
            Expr::Unary(op, expr) => self.unary(op, *expr, insts),
            Expr::DecInc(op, expr) => self.dec_inc(op, *expr, insts),
            Expr::Global(name) => tacky::Val::GlobalVar(name),
            Expr::Local(id) => tacky::Val::Temp(id),
            Expr::Constant(imm) => tacky::Val::Const(imm),
            Expr::Var(_) => unreachable!("Var expr node not resolve"),
            Expr::Poisoned => unreachable!("Attempted to convert poison ast node"),
        }
    }

    fn assign<A: Allocator>(
        &mut self,
        ast_op: AssignOp,
        lhs: Expr<'src, A>,
        rhs: Expr<'src, A>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let rhs = self.expr(rhs, insts);
        let lhs = self.expr(lhs, insts);

        let op = match ast_op {
            AssignOp::Div => tacky::BinaryOp::Div,
            AssignOp::Mul => tacky::BinaryOp::Mul,
            AssignOp::Rem => tacky::BinaryOp::Rem,
            AssignOp::Add => tacky::BinaryOp::Add,
            AssignOp::Sub => tacky::BinaryOp::Sub,
            AssignOp::Shl => tacky::BinaryOp::Shl,
            AssignOp::Shr => tacky::BinaryOp::Sar,
            AssignOp::And => tacky::BinaryOp::BitAnd,
            AssignOp::Or => tacky::BinaryOp::BitOr,
            AssignOp::Xor => tacky::BinaryOp::BitXor,
            AssignOp::Eq => {
                insts.push(tacky::Inst::Copy { src: rhs, dst: lhs });
                return lhs;
            }
        };
        let dst = self.new_temp();
        // Add intermediary binary op for compound assigns
        insts.push(tacky::Inst::Binary { op, lhs, rhs, dst });
        insts.push(tacky::Inst::Copy { src: dst, dst: lhs });
        dst
    }

    fn binary<A: Allocator>(
        &mut self,
        ast_op: BinaryOp,
        lhs: Expr<'src, A>,
        rhs: Expr<'src, A>,
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
        expr: Expr<'src, impl Allocator>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let op = match ast_op {
            UnaryOp::Compliment => tacky::UnaryOp::Compliment,
            UnaryOp::Negate => tacky::UnaryOp::Negate,
            UnaryOp::Not => tacky::UnaryOp::Not,
            UnaryOp::Decrement => {
                return self.assign(AssignOp::Sub, expr, Expr::Constant(1), insts);
            }
            UnaryOp::Increment => {
                return self.assign(AssignOp::Add, expr, Expr::Constant(1), insts);
            }
            UnaryOp::Plus => return self.expr(expr, insts),
        };
        let src = self.expr(expr, insts);
        let dst = self.new_temp();
        insts.push(tacky::Inst::Unary { op, src, dst });
        dst
    }

    fn dec_inc(
        &mut self,
        ast_op: UnaryOp,
        expr: Expr<'src, impl Allocator>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let operand = self.expr(expr, insts);
        let op = match ast_op {
            UnaryOp::Decrement => tacky::BinaryOp::Sub,
            UnaryOp::Increment => tacky::BinaryOp::Add,
            _ => unreachable!("Reached non dec/inc op {ast_op}"),
        };
        let dst = self.new_temp();
        insts.push(tacky::Inst::Copy { src: operand, dst });
        insts.push(tacky::Inst::Binary {
            op,
            lhs: operand,
            rhs: tacky::Val::Const(1),
            dst: operand,
        });
        dst
    }
}
