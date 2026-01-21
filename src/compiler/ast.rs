use crate::{compiler::{asm, token::Token}, intern::InternedStr};

pub struct Program {
    pub(crate) item: Item,
}

pub enum Item {
    Fn { name: String, body: Stmt },
    //Fn { name: InternedStr, body: Stmt },
}

pub enum Stmt {
    Return(Expr),
}

pub enum Expr {
    Constant(i32),
}

impl Program {
    pub fn as_asm(self) -> asm::Program {
        let item = self.item.as_asm();
        asm::Program { item }
    }
}

impl Item {
    fn as_asm(self) -> asm::Item {
        match self {
            Self::Fn { name, body } => {
                let mut insts = Vec::new();
                body.as_asm(&mut insts);
                asm::Item::Fn { name, insts }
            },
        }
    }
}

impl Stmt {
    fn as_asm(self, insts: &mut Vec<asm::Inst>) {
        match self {
            Self::Return(expr) => {
                let src = expr.as_asm(insts);
                insts.push(asm::Inst::Mov { src, dest: asm::Operand::Register });
                insts.push(asm::Inst::Ret);
            },
        }
    }
}

impl Expr {
    fn as_asm(self, insts: &mut Vec<asm::Inst>) -> asm::Operand {
        match self {
            Self::Constant(imm) => asm::Operand::Imm(imm),
        }
    }
}
