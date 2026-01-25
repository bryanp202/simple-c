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

/// Assembly struction abstraction
///
/// All binary inst are stored as (src, dst), similar to AT&T syntax
pub enum Inst {
    // Function
    AllocateStack(usize),
    Ret,
    // Arith
    Add(Operand, Operand),
    Sub(Operand, Operand),
    IMul(Operand, Operand),
    IDiv(Operand),
    Neg(Operand),
    Not(Operand),
    // Other
    Cdq,
    Mov(Operand, Operand),
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Operand {
    Imm(i32),
    Reg(Reg),
    Psuedo(usize),
    Stack(usize),
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Reg {
    AX,
    DX,
    R10,
    R11,
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
                writeln!(f, "    .globl {}", name.get())?;
                writeln!(f, "{}:", name.get())?;
                writeln!(f, "    pushq %rbp")?;
                writeln!(f, "    movq %rsp, %rbp")?;
                for inst in insts {
                    writeln!(f, "    {inst}")?;
                }
                Ok(())
            }
        }
    }
}

impl Display for Inst {
    // All instructions after the first must have "    " (four spaces)
    // Last instruction should be write!(...)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Function
            Self::AllocateStack(amt) => write!(f, "subq ${amt}, %rsp"),
            Self::Ret => {
                writeln!(f, "movq %rbp, %rsp")?;
                writeln!(f, "    popq %rbp")?;
                write!(f, "    ret")
            }
            // Arith
            Self::Add(src, dst) => write!(f, "addl {src}, {dst}"),
            Self::Sub(src, dst) => write!(f, "subl {src}, {dst}"),
            Self::IMul(src, dst) => write!(f, "imull {src}, {dst}"),
            Self::IDiv(operand) => write!(f, "idivl {operand}"),
            Self::Neg(dst) => write!(f, "negl {dst}"),
            Self::Not(dst) => write!(f, "notl {dst}"),
            // Other
            Self::Cdq => write!(f, "cdq"),
            Self::Mov(src, dst) => write!(f, "movl {src}, {dst}"),
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
            Self::DX => "rdx",
            Self::R10 => "r10",
            Self::R11 => "r11",
        }
    }

    fn as_four_byte(&self) -> &'static str {
        match self {
            Self::AX => "eax",
            Self::DX => "edx",
            Self::R10 => "r10d",
            Self::R11 => "r11d",
        }
    }
}
