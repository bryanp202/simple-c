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
        let fixed_asm_program = self.fix(filled_asm_program);
        //let pseudo_asm = self.fill_registers(raw_asm);

        fixed_asm_program
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
            Inst::Unary { op, src, dst } => {
                let src = self.convert_val(src);
                let dst = self.convert_val(dst);
                asm_insts.push(asm::Inst::Mov { src, dst });

                match op {
                    UnaryOp::Compliment => asm::Inst::Not(dst),
                    UnaryOp::Negate => asm::Inst::Neg(dst),
                }
            }
            Inst::Ret(src) => {
                let src = self.convert_val(src);
                asm_insts.push(asm::Inst::Mov {
                    src,
                    dst: asm::Operand::Reg(Reg::AX),
                });
                asm::Inst::Ret
            }
        };

        asm_insts.push(last_inst);
    }

    fn convert_val(&mut self, val: Val) -> asm::Operand {
        match val {
            Val::Const(imm) => asm::Operand::Imm(imm),
            Val::Temp(id) => asm::Operand::Psuedo(id),
            _ => unreachable!(),
        }
    }

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
            asm::Inst::Mov { src, dst } => asm::Inst::Mov {
                src: self.fill_operand(src),
                dst: self.fill_operand(dst),
            },
            asm::Inst::Not(dst) => asm::Inst::Neg(self.fill_operand(dst)),
            asm::Inst::Neg(dst) => asm::Inst::Neg(self.fill_operand(dst)),
            inst => inst,
        }
    }

    fn fill_operand(&mut self, operand: Operand) -> Operand {
        match operand {
            Operand::Psuedo(num) => Operand::Stack(self.reserve_or_get(num, 4, 4)),
            operand => operand,
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
            match inst {
                asm::Inst::Mov { src, dst } => {
                    if src == dst {
                        continue;
                    }

                    if let src @ Operand::Stack(_) = src
                        && let dst @ Operand::Stack(_) = dst
                    {
                        fixed_insts.push(asm::Inst::Mov {
                            src,
                            dst: Operand::Reg(Reg::R10),
                        });
                        fixed_insts.push(asm::Inst::Mov {
                            src: Operand::Reg(Reg::R10),
                            dst,
                        });
                    }
                }

                inst => fixed_insts.push(inst),
            }
        }

        asm::Item::Fn {
            name,
            insts: fixed_insts,
        }
    }
}
