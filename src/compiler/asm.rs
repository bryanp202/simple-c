use std::{fmt::Display, fs::File, io::{self, BufWriter, Write}, path::Path};

use crate::compiler::ast::{self, Expr, Stmt};

pub struct Program {
    pub(crate) item: Item
}

pub enum Item {
    Fn {name: String, insts: Vec<Inst>},
}

pub enum Inst {
    Mov { src: Operand, dest: Operand },
    Ret,
}

pub enum Operand {
    Imm(i32),
    Register
}

impl Program {
    pub fn generate(&self, path: &Path) -> io::Result<()> {
        let mut buf = BufWriter::new(File::create(path)?);
        write!(buf, "{}", self.item)
    }
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fn { name, insts } => {
                writeln!(f, "  .globl {name}")?;
                writeln!(f, "{name}:")?;
                for inst in insts {
                    writeln!(f, "  {inst}")?;
                }
                Ok(())
            },
        }
    }
}

impl Display for Inst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mov { src, dest } => write!(f, "movl {src}, {dest}"),
            Self::Ret => write!(f, "ret"),
        }
    }
}

impl Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Imm(imm) => write!(f, "${imm}"),
            Self::Register => write!(f, "%eax"),
        }
    }
}