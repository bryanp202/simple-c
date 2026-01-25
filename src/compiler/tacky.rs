use crate::{
    compiler::asm::{self, Operand, Reg},
    intern::Interned,
};

pub struct Program<'src> {
    pub(crate) item: Item<'src>,
}

pub enum Item<'src> {
    Fn {
        name: Interned<'src, str>,
        insts: Vec<Inst<'src>>,
    },
}

pub enum Inst<'src> {
    Ret(Val<'src>),
    Unary {
        op: UnaryOp,
        src: Val<'src>,
        dst: Val<'src>,
    },
    Binary {
        op: BinaryOp,
        lhs: Val<'src>,
        rhs: Val<'src>,
        dst: Val<'src>,
    },
}

#[derive(Clone, Copy)]
pub enum Val<'src> {
    Const(i32),
    Var(Interned<'src, str>),
    Temp(usize),
}

pub enum UnaryOp {
    Compliment,
    Negate,
}

#[derive(Debug)]
pub enum BinaryOp {
    Add,
    Sub,
    Div,
    Mul,
    Rem,
    Shl,
    Sar,
    BitAnd,
    BitOr,
    BitXor,
}

pub struct AsmConverter {
    stack: usize,
    registers: Vec<usize>, // Stores the offset of each pseudo register
}

impl Default for AsmConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl AsmConverter {
    pub fn new() -> Self {
        Self {
            stack: 0,
            registers: Vec::new(),
        }
    }

    pub fn convert<'src>(&mut self, tacky: Program<'src>) -> asm::Program<'src> {
        let asm_program = self.convert_program(tacky);
        let filled_asm_program = self.fill_registers(asm_program);
        self.fix(filled_asm_program)
    }
}

impl<'src> AsmConverter {
    fn reserve_or_get(&mut self, pseudo_id: usize, size: usize, align: usize) -> usize {
        if let Some(&pos) = self.registers.get(pseudo_id) {
            return pos;
        }

        // Align the stack to `align` boundary
        self.stack = (self.stack + (align - 1)) & !(align - 1);
        self.stack += size;
        let pos = self.stack;
        self.registers.push(pos);
        pos
    }

    fn convert_program(&mut self, program: Program<'src>) -> asm::Program<'src> {
        asm::Program {
            item: self.convert_item(program.item),
        }
    }

    fn convert_item(&mut self, item: Item<'src>) -> asm::Item<'src> {
        match item {
            Item::Fn { name, insts } => {
                let mut asm_insts = Vec::new();

                for inst in insts {
                    self.convert_inst(inst, &mut asm_insts);
                }

                asm::Item::Fn {
                    name,
                    insts: asm_insts,
                }
            }
        }
    }

    fn convert_inst(&mut self, inst: Inst, asm_insts: &mut Vec<asm::Inst>) {
        let last_inst = match inst {
            Inst::Binary { op, lhs, rhs, dst } => self.convert_binary(op, lhs, rhs, dst, asm_insts),
            Inst::Unary { op, src, dst } => self.convert_unary(op, src, dst, asm_insts),
            Inst::Ret(src) => {
                let src = self.convert_val(src);
                asm_insts.push(asm::Inst::Mov(src, asm::Operand::Reg(Reg::AX)));
                asm::Inst::Ret
            }
        };

        asm_insts.push(last_inst);
    }

    fn convert_binary(
        &mut self,
        op: BinaryOp,
        lhs: Val<'_>,
        rhs: Val<'_>,
        dst: Val<'_>,
        asm_insts: &mut Vec<asm::Inst>,
    ) -> asm::Inst {
        let lhs = self.convert_val(lhs);
        let rhs = self.convert_val(rhs);
        let dst = self.convert_val(dst);

        match op {
            BinaryOp::Div | BinaryOp::Rem => {
                asm_insts.push(asm::Inst::Mov(lhs, Operand::Reg(Reg::AX)));
                asm_insts.push(asm::Inst::Cdq);
                asm_insts.push(asm::Inst::IDiv(rhs));

                let reg = match op {
                    BinaryOp::Div => Reg::AX,
                    BinaryOp::Rem => Reg::DX,
                    _ => unreachable!(),
                };
                asm::Inst::Mov(Operand::Reg(reg), dst)
            }
            BinaryOp::Add | BinaryOp::Mul | BinaryOp::Sub | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                asm_insts.push(asm::Inst::Mov(lhs, dst));
                match op {
                    BinaryOp::Add => asm::Inst::Add(rhs, dst),
                    BinaryOp::Mul => asm::Inst::IMul(rhs, dst),
                    BinaryOp::Sub => asm::Inst::Sub(rhs, dst),
                    BinaryOp::BitAnd => asm::Inst::And(rhs, dst),
                    BinaryOp::BitOr => asm::Inst::Or(rhs, dst),
                    BinaryOp::BitXor => asm::Inst::Xor(rhs, dst),
                    _ => unreachable!(),
                }
            }
            BinaryOp::Shl | BinaryOp::Sar => {
                asm_insts.push(asm::Inst::Mov(lhs, dst));
                match op {
                    BinaryOp::Shl => asm::Inst::Shl(rhs, dst),
                    BinaryOp::Sar => asm::Inst::Sar(rhs, dst),
                    // Safety: Outer pattern only matches these binary ops
                    _ => unreachable!(),
                }
            }
        }
    }

    fn convert_unary(
        &mut self,
        op: UnaryOp,
        src: Val<'_>,
        dst: Val<'_>,
        asm_insts: &mut Vec<asm::Inst>,
    ) -> asm::Inst {
        let src = self.convert_val(src);
        let dst = self.convert_val(dst);
        asm_insts.push(asm::Inst::Mov(src, dst));

        match op {
            UnaryOp::Compliment => asm::Inst::Not(dst),
            UnaryOp::Negate => asm::Inst::Neg(dst),
        }
    }

    fn convert_val(&mut self, val: Val) -> asm::Operand {
        match val {
            Val::Const(imm) => asm::Operand::Imm(imm),
            Val::Temp(id) => asm::Operand::Psuedo(id),
            _ => todo!(),
        }
    }
}

impl<'src> AsmConverter {
    fn fill_registers(&mut self, program: asm::Program<'src>) -> asm::Program<'src> {
        let item = match program.item {
            asm::Item::Fn { name, insts } => self.fill_function(name, insts),
        };

        asm::Program { item }
    }

    fn fill_function(
        &mut self,
        name: Interned<'src, str>,
        insts: Vec<asm::Inst>,
    ) -> asm::Item<'src> {
        let filled_insts = insts.into_iter().map(|inst| self.fill_inst(inst)).collect();
        asm::Item::Fn {
            name,
            insts: filled_insts,
        }
    }

    fn fill_inst(&mut self, inst: asm::Inst) -> asm::Inst {
        match inst {
            // Arith
            asm::Inst::Add(src, dst) => {
                asm::Inst::Add(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::Sub(src, dst) => {
                asm::Inst::Sub(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::IMul(src, dst) => {
                asm::Inst::IMul(self.fill_operand(src), self.fill_operand(dst))
            }
            // Shift
            asm::Inst::Shl(src, dst) => {
                asm::Inst::Shl(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::Sar(src, dst) => {
                asm::Inst::Sar(self.fill_operand(src), self.fill_operand(dst))
            }
            // Bitwise
            asm::Inst::And(src, dst) => {
                asm::Inst::And(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::Or(src, dst) => {
                asm::Inst::Or(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::Xor(src, dst) => {
                asm::Inst::Xor(self.fill_operand(src), self.fill_operand(dst))
            }
            // Special
            asm::Inst::IDiv(operand) => asm::Inst::IDiv(self.fill_operand(operand)),
            asm::Inst::Not(dst) => asm::Inst::Neg(self.fill_operand(dst)),
            asm::Inst::Neg(dst) => asm::Inst::Neg(self.fill_operand(dst)),
            // Other
            asm::Inst::Mov(src, dst) => {
                asm::Inst::Mov(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::AllocateStack(_) | asm::Inst::Cdq | asm::Inst::Ret => inst,
        }
    }

    fn fill_operand(&mut self, operand: Operand) -> Operand {
        match operand {
            Operand::Psuedo(num) => Operand::Stack(self.reserve_or_get(num, 4, 4)),
            Operand::Imm(_) | Operand::Reg(_) | Operand::Stack(_) => operand,
        }
    }

    /// Fixes an illegal memory to memory asm operation
    fn fix_mem_to_mem(
        &mut self,
        src: Operand,
        dst: Operand,
        inst: impl Fn(Operand, Operand) -> asm::Inst,
        fixed_insts: &mut Vec<asm::Inst>,
    ) -> asm::Inst {
        if let Operand::Stack(_) = src
            && let Operand::Stack(_) = dst
        {
            fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::R10)));
            inst(Operand::Reg(Reg::R10), dst)
        } else {
            inst(src, dst)
        }
    }

    fn fix(&mut self, program: asm::Program<'src>) -> asm::Program<'src> {
        let item = match program.item {
            asm::Item::Fn { name, insts } => self.fix_function(name, insts),
        };
        asm::Program { item }
    }

    fn fix_function(
        &mut self,
        name: Interned<'src, str>,
        insts: Vec<asm::Inst>,
    ) -> asm::Item<'src> {
        let mut fixed_insts = vec![asm::Inst::AllocateStack(self.stack)];

        for inst in insts {
            self.fix_inst(inst, &mut fixed_insts);
        }

        asm::Item::Fn {
            name,
            insts: fixed_insts,
        }
    }

    fn fix_inst(&mut self, inst: asm::Inst, fixed_insts: &mut Vec<asm::Inst>) {
        let last_inst = match inst {
            // Arith
            asm::Inst::Add(src, dst) => self.fix_mem_to_mem(src, dst, asm::Inst::Add, fixed_insts),
            asm::Inst::Sub(src, dst) => self.fix_mem_to_mem(src, dst, asm::Inst::Sub, fixed_insts),
            asm::Inst::IMul(src, dst) => {
                if let Operand::Stack(_) = dst {
                    fixed_insts.push(asm::Inst::Mov(dst, Operand::Reg(Reg::R11)));
                    fixed_insts.push(asm::Inst::IMul(src, Operand::Reg(Reg::R11)));
                    asm::Inst::Mov(Operand::Reg(Reg::R11), dst)
                } else {
                    asm::Inst::IMul(src, dst)
                }
            }
            // Shift
            asm::Inst::Shl(src, dst) => {
                if matches!(src, Operand::Imm(imm) if imm > u8::MAX.into())
                    || matches!(src, Operand::Stack(_))
                {
                    fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::CX)));
                    asm::Inst::Shl(Operand::Reg(Reg::CX), dst)
                } else {
                    asm::Inst::Shl(src, dst)
                }
            }
            asm::Inst::Sar(src, dst) => {
                if matches!(src, Operand::Imm(imm) if imm > u8::MAX.into())
                    || matches!(src, Operand::Stack(_))
                {
                    fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::CX)));
                    asm::Inst::Sar(Operand::Reg(Reg::CX), dst)
                } else {
                    asm::Inst::Sar(src, dst)
                }
            }
            // Bitwise
            asm::Inst::And(src, dst) => self.fix_mem_to_mem(src, dst, asm::Inst::And, fixed_insts),
            asm::Inst::Or(src, dst) => self.fix_mem_to_mem(src, dst, asm::Inst::Or, fixed_insts),
            asm::Inst::Xor(src, dst) => self.fix_mem_to_mem(src, dst, asm::Inst::Xor, fixed_insts),
            // Special
            asm::Inst::IDiv(operand) => {
                if let Operand::Imm(_) = operand {
                    fixed_insts.push(asm::Inst::Mov(operand, Operand::Reg(Reg::R10)));
                    asm::Inst::IDiv(Operand::Reg(Reg::R10))
                } else {
                    asm::Inst::IDiv(operand)
                }
            }
            // Other
            asm::Inst::Mov(src, dst) => self.fix_mem_to_mem(src, dst, asm::Inst::Mov, fixed_insts),
            asm::Inst::AllocateStack(_)
            | asm::Inst::Cdq
            | asm::Inst::Neg(_)
            | asm::Inst::Not(_)
            | asm::Inst::Ret => inst,
        };
        fixed_insts.push(last_inst);
    }
}
