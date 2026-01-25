use std::{alloc::Allocator, fmt::Display};

use crate::compiler::ast;

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
            Self::Add => '+',
            Self::Div => '/',
            Self::Mul => '*',
            Self::Rem => '%',
            Self::Sub => '-',
        };
        write!(f, "{c}")
    }
}

impl Display for ast::UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Compliment => '~',
            Self::Negate => '-',
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
