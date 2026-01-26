use crate::{
    compiler::asm::{self, CompareOp, Label, Operand, Reg},
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
    Copy {
        src: Val<'src>,
        dst: Val<'src>,
    },
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
    Jump(Label),
    JumpIfZero(Val<'src>, Label),
    JumpIfNotZero(Val<'src>, Label),
    Label(Label),
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
    Not,
}

#[derive(Debug)]
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
    Sar,
    BitAnd,
    BitXor,
    BitOr,
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
            // Control flow
            Inst::Label(label) => asm::Inst::Label(label),
            Inst::Jump(label) => asm::Inst::Jump(label),
            Inst::JumpIfZero(src, label) => {
                asm_insts.push(asm::Inst::Cmp(Operand::Imm(0), self.convert_val(src)));
                asm::Inst::JumpCC(CompareOp::E, label)
            }
            Inst::JumpIfNotZero(src, label) => {
                asm_insts.push(asm::Inst::Cmp(Operand::Imm(0), self.convert_val(src)));
                asm::Inst::JumpCC(CompareOp::NE, label)
            }
            Inst::Ret(src) => {
                let src = self.convert_val(src);
                asm_insts.push(asm::Inst::Mov(src, asm::Operand::Reg(Reg::AX)));
                asm::Inst::Ret
            }
            // Other
            Inst::Copy { src, dst } => asm::Inst::Mov(self.convert_val(src), self.convert_val(dst)),
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
            // Regular binary ops
            BinaryOp::Add
            | BinaryOp::Mul
            | BinaryOp::Sub
            | BinaryOp::Sar
            | BinaryOp::Shl
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor => {
                asm_insts.push(asm::Inst::Mov(lhs, dst));
                let asm_op = match op {
                    BinaryOp::Add => asm::BinaryOp::Add,
                    BinaryOp::Sub => asm::BinaryOp::Sub,
                    BinaryOp::Mul => asm::BinaryOp::IMul,
                    BinaryOp::Sar => asm::BinaryOp::Sar,
                    BinaryOp::Shl => asm::BinaryOp::Shl,
                    BinaryOp::BitAnd => asm::BinaryOp::And,
                    BinaryOp::BitOr => asm::BinaryOp::Or,
                    BinaryOp::BitXor => asm::BinaryOp::Xor,
                    _ => unreachable!(),
                };
                asm::Inst::Binary(asm_op, rhs, dst)
            }

            // Special register req binary ops
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

            // Compare binary ops
            BinaryOp::E
            | BinaryOp::NE
            | BinaryOp::G
            | BinaryOp::GE
            | BinaryOp::L
            | BinaryOp::LE => {
                let compare_op = match op {
                    BinaryOp::E => CompareOp::E,
                    BinaryOp::NE => CompareOp::NE,
                    BinaryOp::G => CompareOp::G,
                    BinaryOp::GE => CompareOp::GE,
                    BinaryOp::L => CompareOp::L,
                    BinaryOp::LE => CompareOp::LE,
                    _ => unreachable!(),
                };
                asm_insts.push(asm::Inst::Cmp(lhs, rhs));
                asm_insts.push(asm::Inst::Mov(Operand::Imm(0), dst));
                asm::Inst::SetCC(compare_op, dst)
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

        match op {
            UnaryOp::Not => {
                asm_insts.push(asm::Inst::Cmp(Operand::Imm(0), src));
                asm_insts.push(asm::Inst::Mov(Operand::Imm(0), dst));
                asm::Inst::SetCC(CompareOp::E, dst)
            }
            UnaryOp::Compliment | UnaryOp::Negate => {
                asm_insts.push(asm::Inst::Mov(src, dst));
                let asm_op = match op {
                    UnaryOp::Compliment => asm::UnaryOp::Not,
                    UnaryOp::Negate => asm::UnaryOp::Neg,
                    _ => unreachable!(),
                };
                asm::Inst::Unary(asm_op, dst)
            }
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
            asm::Inst::Unary(op, dst) => asm::Inst::Unary(op, self.fill_operand(dst)),
            asm::Inst::Binary(op, src, dst) => {
                asm::Inst::Binary(op, self.fill_operand(src), self.fill_operand(dst))
            }
            // Special
            asm::Inst::IDiv(operand) => asm::Inst::IDiv(self.fill_operand(operand)),
            // Other
            asm::Inst::Mov(src, dst) => {
                asm::Inst::Mov(self.fill_operand(src), self.fill_operand(dst))
            }
            asm::Inst::Cmp(a, b) => asm::Inst::Cmp(self.fill_operand(a), self.fill_operand(b)),
            asm::Inst::SetCC(op, dst) => asm::Inst::SetCC(op, self.fill_operand(dst)),
            // No changes needed
            asm::Inst::AllocateStack(_)
            | asm::Inst::Cdq
            | asm::Inst::Jump(_)
            | asm::Inst::Label(_)
            | asm::Inst::JumpCC(..)
            | asm::Inst::Ret => inst,
        }
    }

    fn fill_operand(&mut self, operand: Operand) -> Operand {
        match operand {
            Operand::Psuedo(num) => Operand::Stack(self.reserve_or_get(num, 4, 4)),
            Operand::Imm(_) | Operand::Reg(_) | Operand::Stack(_) => operand,
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
            asm::Inst::Binary(op, src, dst) => self.fix_binary_inst(op, src, dst, fixed_insts),

            // Prevent imm div operands
            asm::Inst::IDiv(operand) => {
                if let Operand::Imm(_) = operand {
                    fixed_insts.push(asm::Inst::Mov(operand, Operand::Reg(Reg::R10)));
                    asm::Inst::IDiv(Operand::Reg(Reg::R10))
                } else {
                    asm::Inst::IDiv(operand)
                }
            }

            //  Prevent mem to mem move
            asm::Inst::Mov(src, dst) if is_mem_to_mem(&src, &dst) => {
                fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::R10)));
                asm::Inst::Mov(Operand::Reg(Reg::R10), dst)
            }

            // Prevent mem to mem cmp
            asm::Inst::Cmp(a, b) if is_mem_to_mem(&a, &b) => {
                fixed_insts.push(asm::Inst::Mov(a, Operand::Reg(Reg::R10)));
                asm::Inst::Cmp(Operand::Reg(Reg::R10), b)
            }
            // Prevent rhs constant cmp
            asm::Inst::Cmp(a, b) if matches!(b, Operand::Imm(_)) => {
                fixed_insts.push(asm::Inst::Mov(b, Operand::Reg(Reg::R11)));
                asm::Inst::Cmp(a, Operand::Reg(Reg::R11))
            }

            asm::Inst::AllocateStack(_)
            | asm::Inst::Cdq
            | asm::Inst::Cmp(..)
            | asm::Inst::Mov(..)
            | asm::Inst::Unary(..)
            | asm::Inst::Ret
            | asm::Inst::Jump(_)
            | asm::Inst::JumpCC(..)
            | asm::Inst::Label(_)
            | asm::Inst::SetCC(..) => inst,
        };
        fixed_insts.push(last_inst);
    }

    fn fix_binary_inst(
        &mut self,
        op: asm::BinaryOp,
        src: Operand,
        dst: Operand,
        fixed_insts: &mut Vec<asm::Inst>,
    ) -> asm::Inst {
        match op {
            // Prevent memory to memory binary ops
            asm::BinaryOp::Add
            | asm::BinaryOp::Sub
            | asm::BinaryOp::And
            | asm::BinaryOp::Or
            | asm::BinaryOp::Xor
                if is_mem_to_mem(&src, &dst) =>
            {
                fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::R10)));
                asm::Inst::Binary(op, Operand::Reg(Reg::R10), dst)
            }

            // Prevent memory dst for imul
            asm::BinaryOp::IMul if matches!(dst, Operand::Stack(_)) => {
                fixed_insts.push(asm::Inst::Mov(dst, Operand::Reg(Reg::R11)));
                fixed_insts.push(asm::Inst::Binary(op, src, Operand::Reg(Reg::R11)));
                asm::Inst::Mov(Operand::Reg(Reg::R11), dst)
            }

            // Use proper register cl for bit shifting
            asm::BinaryOp::Sar | asm::BinaryOp::Shl
                if matches!(src, Operand::Imm(imm) if imm > u8::MAX.into())
                    || matches!(src, Operand::Stack(_)) =>
            {
                fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::CX)));
                asm::Inst::Binary(op, Operand::Reg(Reg::CX), dst)
            }

            // No fix required
            asm::BinaryOp::Add
            | asm::BinaryOp::And
            | asm::BinaryOp::IMul
            | asm::BinaryOp::Or
            | asm::BinaryOp::Sar
            | asm::BinaryOp::Shl
            | asm::BinaryOp::Sub
            | asm::BinaryOp::Xor => asm::Inst::Binary(op, src, dst),
        }
    }
}

fn is_mem_to_mem(a: &Operand, b: &Operand) -> bool {
    matches!(a, Operand::Stack(_)) && matches!(b, Operand::Stack(_))
}
