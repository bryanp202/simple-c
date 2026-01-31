use std::{
    alloc::Allocator,
    fmt::Display,
    io::{BufWriter, Write},
    path::Path,
};

use crate::compiler::{ast, tacky};

const INDENT_SPACES: usize = 4;

pub trait PrettyPrint {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result;
}

pub fn pretty_print(item: impl Display, name: &'static str, src_path: &Path) {
    let mut buf = BufWriter::new(std::io::stderr());
    write!(&mut buf, "{}: {item}", src_path.display()).unwrap_or_else(|err| {
        eprintln!("{err}: failed to print {name} for: {}", src_path.display())
    });
}

////////////////////////
/// AST PRETTY PRINT ///
////////////////////////
impl<A: Allocator> Display for ast::Program<'_, '_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Program [")?;
        for global in &self.globals {
            global.pretty(f, 1)?;
        }
        for fun in &self.functions {
            fun.pretty(f, 1)?;
        }
        writeln!(f, "]")
    }
}

impl PrettyPrint for ast::GlobalVar<'_> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let ast::GlobalVar { name } = self;
        let spaces = indent * INDENT_SPACES;
        writeln!(f, "{: >spaces$}Global \"{}\"", "", name.get())
    }
}

impl<A: Allocator> PrettyPrint for ast::Function<'_, '_, A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let ast::Function { name, body, .. } = self;
        let spaces = indent * INDENT_SPACES;
        writeln!(f, "{: >spaces$}Fn \"{}\": ", "", name.get())?;
        for stmt in body {
            stmt.pretty(f, indent + 1)?;
            writeln!(f)?;
        }
        writeln!(f, "{: >spaces$}}},", "")
    }
}

impl Display for ast::AssignOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Eq => "=",
            Self::Add => "+=",
            Self::Div => "/=",
            Self::Mul => "*=",
            Self::Rem => "%=",
            Self::Sub => "-=",
            Self::Shl => "<<=",
            Self::Shr => ">>=",
            Self::And => "&=",
            Self::Xor => "^=",
            Self::Or => "|=",
        };
        write!(f, "{c}")
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

impl<A: Allocator> PrettyPrint for ast::Stmt<'_, '_, A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = indent * INDENT_SPACES;
        write!(f, "{: >spaces$}Stmt: ", "")?;
        match self {
            Self::Block(stmts) => {
                writeln!(f, "Block {{")?;
                for stmt in stmts {
                    stmt.pretty(f, indent + 1)?;
                }
            }
            Self::Decl(name, ty, init) => {
                writeln!(f, "Decl {} {} {{", ty.get(), name.get())?;
                if let Some(init) = init {
                    init.pretty(f, indent + 1)?;
                }
            }
            Self::Expr(expr) => {
                writeln!(f, "Expr {{ ")?;
                expr.pretty(f, indent + 1)?;
            }
            Self::Nil => return write!(f, "Nil"),
            Self::Return(expr) => {
                writeln!(f, "Return {{ ")?;
                expr.pretty(f, indent + 1)?;
            }
        }
        write!(f, "{: >spaces$}}},", "")
    }
}

impl<A: Allocator> PrettyPrint for ast::Expr<'_, A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let spaces = indent * INDENT_SPACES;
        write!(f, "{: >spaces$}Expr: ", "")?;
        match self {
            Self::Assign(op, lhs, rhs) => {
                writeln!(f, "Assign{op} {{")?;
                lhs.pretty(f, indent + 1)?;
                rhs.pretty(f, indent + 1)?;
                writeln!(f, "{: >spaces$}}},", "")
            }
            Self::Binary(op, lhs, rhs) => {
                writeln!(f, "Binary{op} {{")?;
                lhs.pretty(f, indent + 1)?;
                rhs.pretty(f, indent + 1)?;
                writeln!(f, "{: >spaces$}}},", "")
            }
            Self::Unary(op, operand) => {
                writeln!(f, "Unary{op} {{")?;
                operand.pretty(f, indent + 1)?;
                writeln!(f, "{: >spaces$}}},", "")
            }
            Self::Global(name) => write!(f, "Global {},", name.get()),
            Self::Local(id) => write!(f, "Local {id},"),
            Self::Var(name) => write!(f, "Var {}", name.get()),
            Self::Constant(imm) => writeln!(f, "Imm {imm},"),
        }
    }
}

//////////////////////////
/// TACKY PRETTY PRINT ///
//////////////////////////
impl Display for tacky::Program<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Program [")?;
        for global in &self.globals {
            global.pretty(f, 1)?;
        }
        for fun in &self.functions {
            fun.pretty(f, 1)?;
        }
        writeln!(f, "]")
    }
}

impl PrettyPrint for tacky::GlobalVar<'_> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let tacky::GlobalVar { name } = self;
        let spaces = indent * INDENT_SPACES;
        writeln!(f, "{: >spaces$}Global \"{}\"", "", name.get())
    }
}

impl PrettyPrint for tacky::Function<'_> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let tacky::Function { name, insts } = self;
        let spaces = indent * INDENT_SPACES;
        writeln!(f, "{: >spaces$}Fn \"{}\": ", "", name.get())?;
        for inst in insts {
            inst.pretty(f, indent + 1)?;
            writeln!(f)?;
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
            Self::GlobalVar(id) => write!(f, "{}", id.get()),
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
            Self::JumpIfNotZero(src, label) => write!(f, "jnz {src} {label}"),
            Self::JumpIfZero(src, label) => write!(f, "jz {src} {label}"),
            Self::Label(label) => write!(f, "{label}"),
            Self::Ret(src) => write!(f, "ret {src}"),
            Self::Unary { op, src, dst } => write!(f, "{dst} <- {op}({src})"),
        }
    }
}
