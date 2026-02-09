use crate::{
    arena::Arena,
    compiler::{
        asm::Label,
        ast::{self, AssignOp, BinaryOp, UnaryOp},
        tacky,
    },
};

type Alloc<'a> = &'a Arena<'static>;
type GlobalVar<'s> = ast::typed::GlobalVar<'s>;
type Program<'s, 'a> = ast::typed::Program<'s, 'a, Alloc<'a>>;
type Function<'s, 'a> = ast::typed::Function<'s, 'a, Alloc<'a>>;
type Stmt<'s, 'a> = ast::typed::Stmt<'s, 'a, Alloc<'a>>;
type Expr<'s, 'a> = ast::typed::Expr<'s, 'a, Alloc<'a>>;
type ExprTy<'s, 'a> = ast::typed::ExprTy<'s, 'a, Alloc<'a>>;

pub struct Converter {
    temp_count: usize,
    label_count: usize,
    curr_local: usize,
    break_label: Label,
    continue_label: Label,
}

impl Default for Converter {
    fn default() -> Self {
        Self::new()
    }
}

impl Converter {
    pub fn new() -> Self {
        Self {
            temp_count: 0,
            label_count: 0,
            curr_local: 0,
            break_label: Label(usize::MAX),
            continue_label: Label(usize::MAX),
        }
    }

    pub fn convert<'src>(&mut self, program: Program<'src, '_>) -> tacky::Program<'src> {
        self.reset_for_program(program.labels);
        let globals = program
            .globals
            .into_iter()
            .map(|global| self.global(global))
            .collect();
        let functions = program
            .functions
            .into_iter()
            .map(|fun| self.function(fun))
            .collect();
        tacky::Program { functions, globals }
    }
}

impl<'src, 'a> Converter {
    #[inline]
    fn reset_for_program(&mut self, labels: usize) {
        self.label_count = labels;
    }

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

    /// Creates a new break label, returns the old one for restoring at end of loop/switch
    #[inline]
    fn new_break_label(&mut self) -> Label {
        let break_label = self.new_label();
        std::mem::replace(&mut self.break_label, break_label)
    }

    /// Creates a new break label, returns the old one for restoring at end of loop
    #[inline]
    fn new_continue_label(&mut self) -> Label {
        let continue_label = self.new_label();
        std::mem::replace(&mut self.continue_label, continue_label)
    }

    fn global(&self, global: GlobalVar<'src>) -> tacky::GlobalVar<'src> {
        let GlobalVar { name } = global;
        tacky::GlobalVar { name }
    }

    fn function(&mut self, fun: Function<'src, 'a>) -> tacky::Function<'src> {
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

    fn stmt(&mut self, stmt: Stmt<'src, 'a>, insts: &mut Vec<tacky::Inst<'src>>) {
        match stmt {
            Stmt::Block(stmts) => {
                for stmt in stmts {
                    self.stmt(stmt, insts);
                }
            }
            Stmt::Break => insts.push(tacky::Inst::Jump(self.break_label)),
            Stmt::Continue => insts.push(tacky::Inst::Jump(self.continue_label)),
            Stmt::Decl(init) => {
                let local = self.new_local();
                if let Some(init) = init {
                    let src = self.expr(*init, insts);
                    insts.push(tacky::Inst::Copy { src, dst: local });
                }
            }
            Stmt::Do(body, condition) => {
                let old_break = self.new_break_label();
                let old_continue = self.new_continue_label();

                let start_label = self.new_label();
                insts.push(tacky::Inst::Label(start_label));
                self.stmt(*body, insts);
                insts.push(tacky::Inst::Label(self.continue_label));
                let cond = self.expr(*condition, insts);
                insts.push(tacky::Inst::JumpIfNotZero(cond, start_label));
                insts.push(tacky::Inst::Label(self.break_label));

                self.break_label = old_break;
                self.continue_label = old_continue;
            }
            Stmt::Expr(expr) => _ = self.expr(*expr, insts),
            Stmt::For(for_stmt) => {
                let old_break = self.new_break_label();
                let old_continue = self.new_continue_label();

                if let Some(init) = for_stmt.init {
                    self.stmt(*init, insts);
                }

                let start_label = self.new_label();
                insts.push(tacky::Inst::Label(start_label));
                if let Some(condition) = for_stmt.condition {
                    let cond = self.expr(*condition, insts);
                    insts.push(tacky::Inst::JumpIfZero(cond, self.break_label));
                }

                self.stmt(*for_stmt.body, insts);

                insts.push(tacky::Inst::Label(self.continue_label));
                if let Some(increment) = for_stmt.increment {
                    _ = self.expr(*increment, insts);
                }

                insts.push(tacky::Inst::Jump(start_label));
                insts.push(tacky::Inst::Label(self.break_label));

                self.break_label = old_break;
                self.continue_label = old_continue;
            }
            Stmt::Goto(label) => insts.push(tacky::Inst::Jump(label)),
            Stmt::If(condition, then_branch, else_branch) => {
                let cond_result = self.expr(*condition, insts);
                let else_label = self.new_label();
                insts.push(tacky::Inst::JumpIfZero(cond_result, else_label));
                self.stmt(*then_branch, insts);

                if let Some(else_branch) = else_branch {
                    let end_label = self.new_label();
                    insts.push(tacky::Inst::Jump(end_label));
                    insts.push(tacky::Inst::Label(else_label));
                    self.stmt(*else_branch, insts);
                    insts.push(tacky::Inst::Label(end_label));
                } else {
                    insts.push(tacky::Inst::Label(else_label));
                }
            }
            Stmt::Labled(label, stmt) => {
                insts.push(tacky::Inst::Label(label));
                self.stmt(*stmt, insts);
            }
            Stmt::Nil => {} // Do nothing
            Stmt::Return(expr) => {
                let src = self.expr(*expr, insts);
                insts.push(tacky::Inst::Ret(src));
            }
            Stmt::Switch(switch_stmt) => {
                let old_break = self.new_break_label();

                let lhs = self.expr(*switch_stmt.expr, insts);

                let op = tacky::BinaryOp::E;
                for case in switch_stmt.cases {
                    let dst = self.new_local();
                    insts.push(tacky::Inst::Binary {
                        op,
                        lhs,
                        rhs: tacky::Val::Const(case.val),
                        dst,
                    });
                    insts.push(tacky::Inst::JumpIfNotZero(dst, case.label));
                }

                if let Some(label) = switch_stmt.default {
                    insts.push(tacky::Inst::Jump(label));
                }

                self.stmt(*switch_stmt.body, insts);

                self.break_label = old_break;
            }
            Stmt::While(condition, body) => {
                let break_label = self.new_label();
                let continue_label = self.new_label();
                let old_break = std::mem::replace(&mut self.break_label, break_label);
                let old_continue = std::mem::replace(&mut self.continue_label, continue_label);

                insts.push(tacky::Inst::Label(self.continue_label));
                let cond = self.expr(*condition, insts);
                insts.push(tacky::Inst::JumpIfZero(cond, self.break_label));

                self.stmt(*body, insts);
                insts.push(tacky::Inst::Jump(self.continue_label));
                insts.push(tacky::Inst::Label(self.break_label));

                self.break_label = old_break;
                self.continue_label = old_continue;
            }
        }
    }

    fn expr(
        &mut self,
        expr: Expr<'src, 'a>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        match expr.expr {
            ExprTy::Ternary(cond, then_branch, else_branch) => {
                self.ternary(*cond, *then_branch, *else_branch, insts)
            }
            ExprTy::Assign(op, lhs, rhs) => self.assign(op, *lhs, *rhs, insts),
            ExprTy::Binary(op, lhs, rhs) => self.binary(op, *lhs, *rhs, insts),
            ExprTy::Unary(op, expr) => self.unary(op, *expr, insts),
            ExprTy::DecInc(op, expr) => self.dec_inc(op, *expr, insts),
            ExprTy::Global(name) => tacky::Val::GlobalVar(name),
            ExprTy::Local(id) => tacky::Val::Temp(id),
            ExprTy::Constant(imm) => tacky::Val::Const(imm),
            ExprTy::Poisoned => unreachable!("Attempted to convert poison ast node"),
        }
    }

    fn ternary(
        &mut self,
        cond: Expr<'src, 'a>,
        then_branch: Expr<'src, 'a>,
        else_branch: Expr<'src, 'a>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let dst = self.new_temp();
        let cond_result = self.expr(cond, insts);
        let else_label = self.new_label();
        insts.push(tacky::Inst::JumpIfZero(cond_result, else_label));

        let then_result = self.expr(then_branch, insts);
        insts.push(tacky::Inst::Copy {
            src: then_result,
            dst,
        });
        let end_label = self.new_label();
        insts.push(tacky::Inst::Jump(end_label));

        insts.push(tacky::Inst::Label(else_label));
        let else_result = self.expr(else_branch, insts);
        insts.push(tacky::Inst::Copy {
            src: else_result,
            dst,
        });
        insts.push(tacky::Inst::Label(end_label));

        dst
    }

    fn assign(
        &mut self,
        ast_op: AssignOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
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

    fn binary(
        &mut self,
        ast_op: BinaryOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
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
        expr: Expr<'src, 'a>,
        insts: &mut Vec<tacky::Inst<'src>>,
    ) -> tacky::Val<'src> {
        let op = match ast_op {
            UnaryOp::Compliment => tacky::UnaryOp::Compliment,
            UnaryOp::Negate => tacky::UnaryOp::Negate,
            UnaryOp::Not => tacky::UnaryOp::Not,
            UnaryOp::Decrement => {
                let rhs = Expr {
                    expr: ExprTy::Constant(1),
                    ty: expr.ty,
                };
                return self.assign(AssignOp::Sub, expr, rhs, insts);
            }
            UnaryOp::Increment => {
                let rhs = Expr {
                    expr: ExprTy::Constant(1),
                    ty: expr.ty,
                };
                return self.assign(AssignOp::Add, expr, rhs, insts);
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
        expr: Expr<'src, 'a>,
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
