use std::{alloc::Allocator, fmt::Display};

const LEVEL_SPACES: usize = 2;

#[derive(Clone, Copy)]
pub struct Level(usize);

impl Level {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn to_spaces(self) -> usize {
        self.0 * LEVEL_SPACES
    }
}

trait Pretty {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, level: Level) -> std::fmt::Result;
}

impl<A: Allocator> Display for super::Program<'_, '_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in &self.items {
            writeln!(f, "{item}")?;
        }
        Ok(())
    }
}

impl<A: Allocator> Display for super::Item<'_, '_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fn(fun) => write!(f, "{fun}"),
            Self::Var(global) => write!(f, "{global}"),
        }
    }
}

impl Display for super::GlobalVar<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "global \"{}\";", self.name.get())
    }
}

impl<A: Allocator> Display for super::Function<'_, '_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name.get();
        writeln!(f, "fn {name}")?;
        block(f, &self.body, Level::new())
    }
}

impl<A: Allocator> Pretty for super::Stmt<'_, '_, A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, level: Level) -> std::fmt::Result {
        let spaces = level.to_spaces();
        write!(f, "{: >spaces$}", "")?;
        match self {
            Self::Block(stmts) => block(f, stmts, level),
            Self::Decl(id, ty, init) => {
                write!(f, "{} {}", ty.get(), id.name.get())?;
                if let Some(init) = init {
                    write!(f, " = {init};")
                } else {
                    write!(f, ";")
                }
            }
            Self::Expr(expr) => write!(f, "{expr};"),
            Self::If(condition, then_branch, else_branch) => {
                writeln!(f, "if ({})", condition)?;
                let then_level = if matches!(then_branch.as_ref(), &Self::Block(_)) {
                    level
                } else {
                    level.next()
                };
                then_branch.pretty(f, then_level)?;
                writeln!(f)?;
                if let Some(else_branch) = else_branch {
                    writeln!(f, "{: >spaces$}else", "")?;
                    let else_level = if matches!(else_branch.as_ref(), &Self::Block(_)) {
                        level
                    } else {
                        level.next()
                    };
                    else_branch.pretty(f, else_level)
                } else {
                    Ok(())
                }
            }
            Self::Nil => write!(f, ";"),
            Self::Return(expr) => write!(f, "return {expr};"),
        }
    }
}

fn block<A: Allocator>(
    f: &mut std::fmt::Formatter<'_>,
    stmts: &[super::Stmt<'_, '_, A>],
    level: Level,
) -> std::fmt::Result {
    let spaces = level.to_spaces();
    let next_level = level.next();
    writeln!(f, "{{")?;
    for stmt in stmts {
        stmt.pretty(f, next_level)?;
        writeln!(f)?;
    }
    write!(f, "{: >spaces$}}}", "")
}

impl<A: Allocator> Display for super::Expr<'_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.expr {
            super::ExprTy::Assign(op, lhs, rhs) => write!(f, "({lhs} {op} {rhs})"),
            super::ExprTy::Binary(op, lhs, rhs) => write!(f, "({lhs} {op} {rhs})"),
            super::ExprTy::Constant(imm) => write!(f, "{imm}"),
            super::ExprTy::DecInc(op, operand) => write!(f, "({op}{operand})"),
            super::ExprTy::Poisoned => write!(f, "()"),
            super::ExprTy::Ternary(condition, else_branch, then_branch) => {
                write!(f, "({condition} ? {else_branch} : {then_branch})")
            }
            super::ExprTy::Unary(op, operand) => write!(f, "({op}{operand})"),
            super::ExprTy::Var(id) => write!(f, "{}", id.get()),
        }
    }
}

impl Display for super::AssignOp {
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

impl Display for super::BinaryOp {
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

impl Display for super::UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = match self {
            Self::Compliment => "~",
            Self::Plus => "+",
            Self::Negate => "-",
            Self::Not => "!",
            Self::Increment => "++",
            Self::Decrement => "--",
        };
        write!(f, "{c}")
    }
}
