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
    Shl(Operand, Operand),
    Shr(Operand, Operand),
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

pub struct OneByteOperand<'o>(&'o Operand);
pub struct TwoByteOperand<'o>(&'o Operand);
pub struct FourByteOperand<'o>(&'o Operand);
pub struct EightByteOperand<'o>(&'o Operand);

impl Operand {
    /// Display as one byte registers
    fn display_b(&self) -> OneByteOperand<'_> {
        OneByteOperand(self)
    }

    /// Display with two byte registers
    fn display_w(&self) -> TwoByteOperand<'_> {
        TwoByteOperand(self)
    }

    /// Display with four byte registers
    fn display_d(&self) -> FourByteOperand<'_> {
        FourByteOperand(self)
    }

    /// Display with eight byte registers
    fn display_q(&self) -> EightByteOperand<'_> {
        EightByteOperand(self)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Reg {
    AX,
    CX,
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
            Self::Add(src, dst) => write!(f, "addl {}, {}", src.display_d(), dst.display_d()),
            Self::Sub(src, dst) => write!(f, "subl {}, {}", src.display_d(), dst.display_d()),
            Self::IMul(src, dst) => write!(f, "imull {}, {}", src.display_d(), dst.display_d()),
            Self::Shl(src, dst) => write!(f, "shl {}, {}", src.display_b(), dst.display_d()),
            Self::Shr(src, dst) => write!(f, "shr {}, {}", src.display_b(), dst.display_d()),
            Self::IDiv(operand) => write!(f, "idivl {}", operand.display_d()),
            Self::Neg(dst) => write!(f, "negl {}", dst.display_d()),
            Self::Not(dst) => write!(f, "notl {}", dst.display_d()),
            // Other
            Self::Cdq => write!(f, "cdq"),
            Self::Mov(src, dst) => write!(f, "movl {}, {}", src.display_d(), dst.display_d()),
        }
    }
}

impl Display for OneByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_one_byte()),
            Operand::Stack(offset) => write!(f, "-{offset}(%rsp)"),
            _ => unreachable!(),
        }
    }
}

impl Display for TwoByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_two_byte()),
            Operand::Stack(offset) => write!(f, "-{offset}(%rsp)"),
            _ => unreachable!(),
        }
    }
}

impl Display for FourByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_four_byte()),
            Operand::Stack(offset) => write!(f, "-{offset}(%rsp)"),
            _ => unreachable!(),
        }
    }
}

impl Display for EightByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_eight_byte()),
            Operand::Stack(offset) => write!(f, "-{offset}(%rsp)"),
            _ => unreachable!(),
        }
    }
}

impl Reg {
    fn as_one_byte(&self) -> &'static str {
        match self {
            Self::AX => "al",
            Self::CX => "cl",
            Self::DX => "dl",
            Self::R10 => "r10b",
            Self::R11 => "r11b",
        }
    }

    fn as_two_byte(&self) -> &'static str {
        match self {
            Self::AX => "ax",
            Self::CX => "cx",
            Self::DX => "dx",
            Self::R10 => "r10w",
            Self::R11 => "r11w",
        }
    }

    fn as_four_byte(&self) -> &'static str {
        match self {
            Self::AX => "eax",
            Self::CX => "ecx",
            Self::DX => "edx",
            Self::R10 => "r10d",
            Self::R11 => "r11d",
        }
    }

    fn as_eight_byte(&self) -> &'static str {
        match self {
            Self::AX => "rax",
            Self::CX => "rcx",
            Self::DX => "rdx",
            Self::R10 => "r10",
            Self::R11 => "r11",
        }
    }
}
