use std::{
    fmt::Display,
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use crate::intern::Interned;

pub enum Linkage {
    Global,
    None,
}

pub struct Program<'src> {
    pub(crate) functions: Vec<Function<'src>>,
    pub(crate) globals: Vec<GlobalVar<'src>>,
}

pub struct GlobalVar<'src> {
    pub(crate) linkage: Linkage,
    pub(crate) name: Interned<'src, str>,
    pub(crate) generation: Option<u32>,
    pub(crate) def: Option<i32>,
}

pub struct Function<'src> {
    pub(crate) linkage: Linkage,
    pub(crate) name: Interned<'src, str>,
    pub(crate) insts: Vec<Inst<'src>>,
}

/// Assembly struction abstraction
///
/// All binary inst are stored as (src, dst), similar to AT&T syntax
pub enum Inst<'src> {
    Unary(UnaryOp, Operand<'src>),
    Binary(BinaryOp, Operand<'src>, Operand<'src>),
    // Control flow
    Call(Operand<'src>),
    Label(Label),
    Jump(Label),
    JumpCC(CompareOp, Label),
    // Other
    Cmp(Operand<'src>, Operand<'src>),
    SetCC(CompareOp, Operand<'src>),
    Cdq,
    Mov(Operand<'src>, Operand<'src>),
    Pop(Operand<'src>),
    Push(Operand<'src>),
    // Special
    IDiv(Operand<'src>),
    // Function
    AllocateStack(usize),
    DeallocateStack(usize),
    Ret,
}

#[derive(Clone, Copy)]
pub struct Label(pub(crate) u32);

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Operand<'src> {
    Imm(i32),
    Reg(Reg),
    Psuedo(u32),
    Stack(isize),
    Fn(Interned<'src, str>),
    Data(Interned<'src, str>),
    LocalData(u32),
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum BinaryOp {
    Add,
    Sub,
    IMul,
    Shl,
    Sar,
    And,
    Or,
    Xor,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum CompareOp {
    E,
    NE,
    L,
    LE,
    G,
    GE,
}

pub struct OneByteOperand<'o>(&'o Operand<'o>);
pub struct TwoByteOperand<'o>(&'o Operand<'o>);
pub struct FourByteOperand<'o>(&'o Operand<'o>);
pub struct EightByteOperand<'o>(&'o Operand<'o>);

impl Operand<'_> {
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
    R8,
    R9,
    R10,
    R11,
}

impl Program<'_> {
    pub fn generate(&self, path: &Path) -> io::Result<()> {
        let mut buf = BufWriter::new(File::create(path)?);
        write!(buf, "{self}")
    }
}

impl Display for Program<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "    .text")?;
        for fun in &self.functions {
            writeln!(f, "{fun}")?;
        }

        for global in &self.globals {
            writeln!(f, "{global}")?;
        }

        Ok(())
    }
}

impl Display for GlobalVar<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(local_gen) = self.generation {
            if matches!(self.linkage, Linkage::Global) {
                writeln!(f, "    .globl LS{local_gen}")?;
            }

            if let Some(def) = self.def {
                writeln!(f, "    .data")?;
                writeln!(f, "    .align 4")?;
                writeln!(f, "LD{local_gen}:")?;
                writeln!(f, "    .long {def}")
            } else {
                writeln!(f, "    .bss")?;
                writeln!(f, "    .align 4")?;
                writeln!(f, "LD{local_gen}:")?;
                writeln!(f, "    .zero 4")
            }
        } else {
            if matches!(self.linkage, Linkage::Global) {
                writeln!(f, "    .globl {}", self.name)?;
            }

            if let Some(def) = self.def {
                writeln!(f, "    .data")?;
                writeln!(f, "    .align 4")?;
                writeln!(f, "{}:", self.name)?;
                writeln!(f, "    .long {def}")
            } else {
                writeln!(f, "    .bss")?;
                writeln!(f, "    .align 4")?;
                writeln!(f, "{}:", self.name)?;
                writeln!(f, "    .zero 4")
            }
        }
    }
}

impl Display for Function<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if matches!(self.linkage, Linkage::Global) {
            writeln!(f, "    .globl {}", self.name)?;
        }
        writeln!(f, "{}:", self.name)?;
        writeln!(f, "    pushq %rbp")?;
        writeln!(f, "    movq %rsp, %rbp")?;
        for inst in &self.insts {
            writeln!(f, "{inst}")?;
        }
        Ok(())
    }
}

impl Display for Inst<'_> {
    // All instructions after the first must have "    " (four spaces)
    // Last instruction should be write!(...)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !matches!(self, Self::Label(_)) {
            write!(f, "    ")?;
        }

        match self {
            Self::Unary(op, dst) => write!(f, "{op} {}", dst.display_d()),
            Self::Binary(op, src, dst) if matches!(op, BinaryOp::Sar | BinaryOp::Shl) => {
                write!(f, "{op} {}, {}", src.display_b(), dst.display_d())
            }
            Self::Binary(op, src, dst) => {
                write!(f, "{op} {}, {}", src.display_d(), dst.display_d())
            }
            // Control flow
            Self::Call(fun) => write!(f, "call {}", fun.display_q()),
            Self::Label(label) => write!(f, "{label}:"),
            Self::Jump(label) => write!(f, "jmp {label}"),
            Self::JumpCC(cc, label) => write!(f, "j{cc} {label}"),
            // Special
            Self::IDiv(operand) => write!(f, "idivl {}", operand.display_d()),
            // Other
            Self::Cmp(a, b) => write!(f, "cmpl {}, {}", a.display_d(), b.display_d()),
            Self::SetCC(cc, dst) => write!(f, "set{cc} {}", dst.display_b()),
            Self::Cdq => write!(f, "cdq"),
            Self::Mov(src, dst) => write!(f, "movl {}, {}", src.display_d(), dst.display_d()),
            Self::Pop(dst) => write!(f, "popq {}", dst.display_q()),
            Self::Push(src) => write!(f, "pushq {}", src.display_q()),
            // Function
            Self::AllocateStack(amt) => write!(f, "subq ${amt}, %rsp"),
            Self::DeallocateStack(amt) => write!(f, "addq ${amt}, %rsp"),
            Self::Ret => {
                writeln!(f, "movq %rbp, %rsp")?;
                writeln!(f, "    popq %rbp")?;
                write!(f, "    ret")
            }
        }
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, ".L{}", self.0)
    }
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op = match self {
            Self::Neg => "negl",
            Self::Not => "notl",
        };
        write!(f, "{op}")
    }
}

impl Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op = match self {
            Self::Add => "addl",
            Self::Sub => "subl",
            Self::IMul => "imull",
            // Shift
            Self::Shl => "shll",
            Self::Sar => "sarl",
            // Bitwise
            Self::And => "andl",
            Self::Or => "orl",
            Self::Xor => "xorl",
        };
        write!(f, "{op}")
    }
}

impl Display for CompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op = match self {
            Self::E => "e",
            Self::NE => "ne",
            Self::G => "g",
            Self::GE => "ge",
            Self::L => "l",
            Self::LE => "le",
        };
        write!(f, "{op}")
    }
}

impl Display for OneByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_one_byte()),
            Operand::Stack(offset) => write!(f, "{offset}(%rbp)"),
            Operand::Fn(name) => write!(f, "{name}"),
            Operand::Data(name) => write!(f, "{name}(%rip)"),
            Operand::LocalData(id) => write!(f, "LD{id}(%rip)"),
            Operand::Psuedo(_) => unreachable!(),
        }
    }
}

impl Display for TwoByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_two_byte()),
            Operand::Stack(offset) => write!(f, "{offset}(%rbp)"),
            Operand::Fn(name) => write!(f, "{name}"),
            Operand::Data(name) => write!(f, "{name}(%rip)"),
            Operand::LocalData(id) => write!(f, "LD{id}(%rip)"),
            Operand::Psuedo(_) => unreachable!(),
        }
    }
}

impl Display for FourByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_four_byte()),
            Operand::Stack(offset) => write!(f, "{offset}(%rbp)"),
            Operand::Fn(name) => write!(f, "{name}"),
            Operand::Data(name) => write!(f, "{name}(%rip)"),
            Operand::LocalData(id) => write!(f, "LD{id}(%rip)"),
            Operand::Psuedo(_) => unreachable!(),
        }
    }
}

impl Display for EightByteOperand<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Operand::Imm(imm) => write!(f, "${imm}"),
            Operand::Reg(reg) => write!(f, "%{}", reg.as_eight_byte()),
            Operand::Stack(offset) => write!(f, "{offset}(%rbp)"),
            Operand::Fn(name) => write!(f, "{name}"),
            Operand::Data(name) => write!(f, "{name}(%rip)"),
            Operand::LocalData(id) => write!(f, "LD{id}(%rip)"),
            Operand::Psuedo(_) => unreachable!(),
        }
    }
}

impl Reg {
    fn as_one_byte(self) -> &'static str {
        match self {
            Self::AX => "al",
            Self::CX => "cl",
            Self::DX => "dl",
            Self::R8 => "r8b",
            Self::R9 => "r9b",
            Self::R10 => "r10b",
            Self::R11 => "r11b",
        }
    }

    fn as_two_byte(self) -> &'static str {
        match self {
            Self::AX => "ax",
            Self::CX => "cx",
            Self::DX => "dx",
            Self::R8 => "r8w",
            Self::R9 => "r9w",
            Self::R10 => "r10w",
            Self::R11 => "r11w",
        }
    }

    fn as_four_byte(self) -> &'static str {
        match self {
            Self::AX => "eax",
            Self::CX => "ecx",
            Self::DX => "edx",
            Self::R8 => "r8d",
            Self::R9 => "r9d",
            Self::R10 => "r10d",
            Self::R11 => "r11d",
        }
    }

    fn as_eight_byte(self) -> &'static str {
        match self {
            Self::AX => "rax",
            Self::CX => "rcx",
            Self::DX => "rdx",
            Self::R8 => "r8",
            Self::R9 => "r9",
            Self::R10 => "r10",
            Self::R11 => "r11",
        }
    }
}
