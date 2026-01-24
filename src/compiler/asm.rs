use std::{
    fmt::Display,
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use crate::intern::Interned;

pub struct Program<'src> {
    pub(crate) item: Item<'src>,
}

pub enum Item<'src> {
    Fn {
        name: Interned<'src, str>,
        insts: Vec<Inst>,
    },
}

pub enum Inst {
    AllocateStack(usize),
    Neg(Operand),
    Not(Operand),
    Mov { src: Operand, dst: Operand },
    Ret,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Operand {
    Imm(i32),
    Reg(Reg),
    Psuedo(usize),
    Stack(usize),
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Reg {
    AX,
    R10,
}

impl<'src> Program<'src> {
    pub fn generate(&self, path: &Path) -> io::Result<()> {
        let mut buf = BufWriter::new(File::create(path)?);
        write!(buf, "{}", self)
    }
}

impl<'src> Display for Program<'src> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.item)
    }
}

impl<'src> Display for Item<'src> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fn { name, insts } => {
                writeln!(f, "  .globl {}", name.get())?;
                writeln!(f, "{}:", name.get())?;
                writeln!(f, "  pushq %rbp")?;
                writeln!(f, "  movq %rsp, %rbp")?;
                for inst in insts {
                    writeln!(f, "  {inst}")?;
                }
                Ok(())
            }
        }
    }
}

impl Display for Inst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AllocateStack(amt) => write!(f, "subq ${amt}, %rsp"),
            Self::Mov { src, dst } => write!(f, "movl {src}, {dst}"),
            Self::Neg(dst) => write!(f, "negl {dst}"),
            Self::Not(dst) => write!(f, "notl {dst}"),
            Self::Ret => {
                writeln!(f, "movq %rbp, %rsp")?;
                writeln!(f, "  popq %rbp")?;
                write!(f, "  ret")
            }
        }
    }
}

impl Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Imm(imm) => write!(f, "${imm}"),
            Self::Reg(reg) => write!(f, "%{}", reg.as_four_byte()),
            Self::Stack(offset) => write!(f, "-{offset}(%rsp)"),
            _ => unreachable!(),
        }
    }
}

impl Reg {
    fn as_eight_byte(&self) -> &'static str {
        match self {
            Self::AX => "rax",
            Self::R10 => "r10",
        }
    }

    fn as_four_byte(&self) -> &'static str {
        match self {
            Self::AX => "eax",
            Self::R10 => "r10d",
        }
    }
}
