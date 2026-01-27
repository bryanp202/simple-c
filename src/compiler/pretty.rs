use std::{alloc::Allocator, fmt::Display};

use crate::compiler::{ast, tacky};

const INDENT_SPACES: usize = 4;

pub trait PrettyPrint {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result;
}

////////////////////////
/// AST PRETTY PRINT ///
////////////////////////
impl<A: Allocator> Display for ast::Program<'_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Program [")?;
        self.item.pretty(f, 1)?;
        writeln!(f, "]")
    }
}

impl Display for ast::BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Add => "+",
            Self::Div => "/",
            Self::Mul => "*",
            Self::Rem => "%",
            Self::Sub => "-",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::G => ">",
            Self::GE => ">=",
            Self::L => "<",
            Self::LE => "<=",
            Self::E => "==",
            Self::NE => "!=",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::And => "&&",
            Self::Or => "||",
        };
        write!(f, "{c}")
    }
}

impl Display for ast::UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Compliment => "~",
            Self::Negate => "-",
            Self::Not => "!",
        };
        write!(f, "{c}")
    }
}

impl<A: Allocator> PrettyPrint for ast::Item<'_, A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = indent * INDENT_SPACES;
        write!(f, "{: >spaces$}Item: ", "")?;
        match self {
            Self::Fn { name, body } => {
                writeln!(f, "Fn \"{}\" {{", name.get())?;
                body.pretty(f, indent + 1)?;
            }
        }
        writeln!(f, "{: >spaces$}}},", "")
    }
}

impl<A: Allocator> PrettyPrint for ast::Stmt<A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = indent * INDENT_SPACES;
        write!(f, "{: >spaces$}Stmt: ", "")?;
        match self {
            Self::Return(expr) => {
                writeln!(f, "Return {{ ")?;
                expr.pretty(f, indent + 1)?;
            }
        }
        writeln!(f, "{: >spaces$}}},", "")
    }
}

impl<A: Allocator> PrettyPrint for ast::Expr<A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = indent * INDENT_SPACES;
        write!(f, "{: >spaces$}Expr: ", "")?;
        match self {
            Self::Binary(op, lhs, rhs) => {
                writeln!(f, "Binary{op} {{")?;
                lhs.pretty(f, indent + 1)?;
                rhs.pretty(f, indent + 1)?;
                writeln!(f, "{: >spaces$}}},", "")
            }
            Self::Constant(imm) => {
                writeln!(f, "Imm {imm},")
            }
            Self::Unary(op, operand) => {
                writeln!(f, "Unary{op} {{")?;
                operand.pretty(f, indent + 1)?;
                writeln!(f, "{: >spaces$}}},", "")
            }
        }
    }
}

//////////////////////////
/// TACKY PRETTY PRINT ///
//////////////////////////
impl Display for tacky::Program<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Program [")?;
        self.item.pretty(f, 1)?;
        writeln!(f, "]")
    }
}

impl PrettyPrint for tacky::Item<'_> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = indent * INDENT_SPACES;
        write!(f, "{: >spaces$}Item: ", "")?;
        match self {
            Self::Fn { name, insts } => {
                writeln!(f, "Fn \"{}\" {{", name.get())?;
                for inst in insts {
                    inst.pretty(f, indent + 1)?;
                    writeln!(f)?;
                }
            }
        }
        writeln!(f, "{: >spaces$}}},", "")
    }
}

impl Display for tacky::BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op = match self {
            Self::Add => "+",
            Self::Div => "/",
            Self::Mul => "*",
            Self::Rem => "%",
            Self::Sub => "-",
            Self::Shl => "<<",
            Self::Sar => ">>",
            Self::G => ">",
            Self::GE => ">=",
            Self::L => "<",
            Self::LE => "<=",
            Self::E => "==",
            Self::NE => "!=",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
        };
        write!(f, "{op}")
    }
}

impl Display for tacky::UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Compliment => "~",
            Self::Negate => "-",
            Self::Not => "!",
        };
        write!(f, "{c}")
    }
}

impl Display for tacky::Val<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Const(imm) => write!(f, "${imm}"),
            Self::Temp(id) => write!(f, ".tmp{id}"),
            Self::Global(id) => write!(f, "{}", id.get()),
        }
    }
}

impl PrettyPrint for tacky::Inst<'_> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = match self {
            Self::Label(_) => indent.saturating_sub(1) * INDENT_SPACES,
            _ => indent * INDENT_SPACES,
        };
        write!(f, "{: >spaces$}", "")?;
        match self {
            Self::Binary { op, lhs, rhs, dst } => {
                write!(f, "{dst} <- {lhs} {op} {rhs}")
            }
            Self::Copy { src, dst } => write!(f, "{dst} <- {src}"),
            Self::Jump(label) => write!(f, "jmp {label}"),
            Self::JumpIfNotZero(src, label) => write!(f, "jmp {label} if {src} != 0"),
            Self::JumpIfZero(src, label) => write!(f, "jmp {label} if {src} == 0"),
            Self::Label(label) => write!(f, "{label}"),
            Self::Ret(src) => write!(f, "ret {src}"),
            Self::Unary { op, src, dst } => write!(f, "{dst} <- {op}({src})"),
        }
    }
}
