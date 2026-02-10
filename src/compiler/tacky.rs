use crate::{
    compiler::asm::{self, CompareOp, Label, Operand, Reg},
    intern::Interned,
};

mod pretty;

pub struct Program<'src> {
    pub(crate) functions: Vec<Function<'src>>,
    pub(crate) globals: Vec<GlobalVar<'src>>,
}

pub struct GlobalVar<'src> {
    pub(crate) name: Interned<'src, str>,
}

pub struct Function<'src> {
    pub(crate) name: Interned<'src, str>,
    pub(crate) insts: Vec<Inst<'src>>,
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
    GlobalVar(Interned<'src, str>),
    Temp(usize),
}

#[derive(Clone, Copy)]
pub enum UnaryOp {
    Compliment,
    Negate,
    Not,
}

#[derive(Clone, Copy, Debug)]
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
    registers: Vec<Option<usize>>, // Stores the offset of each pseudo register
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
    fn reset_for_fn(&mut self) {
        self.stack = 0;
        self.registers.clear();
    }

    fn reserve_or_get(&mut self, pseudo_id: usize, size: usize, align: usize) -> usize {
        if let Some(&Some(pos)) = self.registers.get(pseudo_id) {
            return pos;
        }

        // Align the stack to `align` boundary
        self.stack = (self.stack + (align - 1)) & !(align - 1);
        self.stack += size;
        let pos = self.stack;

        // Check if temp registers are being accessed out of order
        if self.registers.len() <= pseudo_id {
            self.registers.resize(pseudo_id + 1, None);
        }

        self.registers[pseudo_id] = Some(pos);
        pos
    }

    fn convert_program(&mut self, program: Program<'src>) -> asm::Program<'src> {
        asm::Program {
            globals: program
                .globals
                .into_iter()
                .map(|global| self.convert_global(global))
                .collect(),
            functions: program
                .functions
                .into_iter()
                .map(|fun| self.convert_fun(fun))
                .collect(),
        }
    }

    fn convert_global(&mut self, global: GlobalVar<'src>) -> asm::GlobalVar<'src> {
        let GlobalVar { name } = global;
        asm::GlobalVar { name }
    }

    fn convert_fun(&mut self, fun: Function<'src>) -> asm::Function<'src> {
        let Function { name, insts } = fun;
        let mut asm_insts = Vec::new();

        for inst in insts {
            self.convert_inst(inst, &mut asm_insts);
        }

        asm::Function {
            name,
            insts: asm_insts,
        }
    }

    fn convert_inst(&mut self, inst: Inst<'src>, asm_insts: &mut Vec<asm::Inst<'src>>) {
        let last_inst = match inst {
            Inst::Binary { op, lhs, rhs, dst } => {
                Self::convert_binary(op, lhs, rhs, dst, asm_insts)
            }
            Inst::Unary { op, src, dst } => Self::convert_unary(op, src, dst, asm_insts),
            // Control flow
            Inst::Label(label) => asm::Inst::Label(label),
            Inst::Jump(label) => asm::Inst::Jump(label),
            Inst::JumpIfZero(src, label) => {
                asm_insts.push(asm::Inst::Cmp(Operand::Imm(0), Self::convert_val(src)));
                asm::Inst::JumpCC(CompareOp::E, label)
            }
            Inst::JumpIfNotZero(src, label) => {
                asm_insts.push(asm::Inst::Cmp(Operand::Imm(0), Self::convert_val(src)));
                asm::Inst::JumpCC(CompareOp::NE, label)
            }
            Inst::Ret(src) => {
                let src = Self::convert_val(src);
                asm_insts.push(asm::Inst::Mov(src, asm::Operand::Reg(Reg::AX)));
                asm::Inst::Ret
            }
            // Other
            Inst::Copy { src, dst } => {
                asm::Inst::Mov(Self::convert_val(src), Self::convert_val(dst))
            }
        };

        asm_insts.push(last_inst);
    }

    fn convert_binary(
        op: BinaryOp,
        lhs: Val<'src>,
        rhs: Val<'src>,
        dst: Val<'src>,
        asm_insts: &mut Vec<asm::Inst<'src>>,
    ) -> asm::Inst<'src> {
        let lhs = Self::convert_val(lhs);
        let rhs = Self::convert_val(rhs);
        let dst = Self::convert_val(dst);

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
                asm_insts.push(asm::Inst::Cmp(rhs, lhs));
                asm_insts.push(asm::Inst::Mov(Operand::Imm(0), dst));
                asm::Inst::SetCC(compare_op, dst)
            }
        }
    }

    fn convert_unary(
        op: UnaryOp,
        src: Val<'src>,
        dst: Val<'src>,
        asm_insts: &mut Vec<asm::Inst<'src>>,
    ) -> asm::Inst<'src> {
        let src = Self::convert_val(src);
        let dst = Self::convert_val(dst);

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
                    UnaryOp::Not => unreachable!(),
                };
                asm::Inst::Unary(asm_op, dst)
            }
        }
    }

    fn convert_val(val: Val) -> asm::Operand {
        match val {
            Val::Const(imm) => asm::Operand::Imm(imm),
            Val::Temp(id) => asm::Operand::Psuedo(id),
            Val::GlobalVar(_) => todo!(),
        }
    }
}

impl<'src> AsmConverter {
    fn fill_registers(&mut self, program: asm::Program<'src>) -> asm::Program<'src> {
        let asm::Program { globals, functions } = program;
        let globals = globals
            .into_iter()
            .map(|global| self.fill_global(global))
            .collect();
        let functions = functions
            .into_iter()
            .map(|fun| self.fill_function(fun))
            .collect();

        asm::Program { functions, globals }
    }

    fn fill_global(&mut self, global: asm::GlobalVar<'src>) -> asm::GlobalVar<'src> {
        global
    }

    fn fill_function(&mut self, fun: asm::Function<'src>) -> asm::Function<'src> {
        self.reset_for_fn();
        let asm::Function { name, insts } = fun;
        let filled_insts = insts.into_iter().map(|inst| self.fill_inst(inst)).collect();
        asm::Function {
            name,
            insts: filled_insts,
        }
    }

    fn fill_inst(&mut self, inst: asm::Inst<'src>) -> asm::Inst<'src> {
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

    fn fill_operand(&mut self, operand: Operand<'src>) -> Operand<'src> {
        match operand {
            Operand::Psuedo(num) => Operand::Stack(self.reserve_or_get(num, 4, 4)),
            Operand::Imm(_) | Operand::Reg(_) | Operand::Stack(_) | Operand::GlobalVar(_) => {
                operand
            }
        }
    }
}

impl<'src> AsmConverter {
    fn fix(&mut self, program: asm::Program<'src>) -> asm::Program<'src> {
        let asm::Program { globals, functions } = program;
        let functions = functions
            .into_iter()
            .map(|fun| self.fix_function(fun))
            .collect();

        asm::Program { functions, globals }
    }

    fn fix_function(&self, fun: asm::Function<'src>) -> asm::Function<'src> {
        let asm::Function { name, insts } = fun;
        let mut fixed_insts = Vec::with_capacity(insts.capacity());
        fixed_insts.push(asm::Inst::AllocateStack(self.stack));

        for inst in insts {
            Self::fix_inst(inst, &mut fixed_insts);
        }

        asm::Function {
            name,
            insts: fixed_insts,
        }
    }

    fn fix_inst(inst: asm::Inst<'src>, fixed_insts: &mut Vec<asm::Inst<'src>>) {
        let last_inst = match inst {
            asm::Inst::Binary(op, src, dst) => Self::fix_binary_inst(op, src, dst, fixed_insts),

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
            asm::Inst::Mov(src, dst) if is_mem_to_mem(src, dst) => {
                fixed_insts.push(asm::Inst::Mov(src, Operand::Reg(Reg::R10)));
                asm::Inst::Mov(Operand::Reg(Reg::R10), dst)
            }

            // Prevent mem to mem cmp
            asm::Inst::Cmp(a, b) if is_mem_to_mem(a, b) => {
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
        op: asm::BinaryOp,
        src: Operand<'src>,
        dst: Operand<'src>,
        fixed_insts: &mut Vec<asm::Inst<'src>>,
    ) -> asm::Inst<'src> {
        match op {
            // Prevent memory to memory binary ops
            asm::BinaryOp::Add
            | asm::BinaryOp::Sub
            | asm::BinaryOp::And
            | asm::BinaryOp::Or
            | asm::BinaryOp::Xor
                if is_mem_to_mem(src, dst) =>
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

fn is_mem_to_mem(a: Operand, b: Operand) -> bool {
    matches!(a, Operand::Stack(_)) && matches!(b, Operand::Stack(_))
}
