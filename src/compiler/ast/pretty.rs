use std::{alloc::Allocator, fmt::Display};

const LEVEL_SPACES: usize = 4;

#[derive(Clone, Copy)]
pub struct Level(usize);

impl Level {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn prev(self) -> Self {
        Self(self.0.saturating_sub(1))
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
            Self::FnDecl(decl) => write!(f, "{decl}"),
            Self::FnDef(def) => write!(f, "{def}"),
            Self::Var(global) => write!(f, "{global}"),
        }
    }
}

impl Display for super::GlobalVar<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "global \"{}\";", self.id.name)
    }
}

impl Display for super::FunctionDecl<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {};", self.ty, self.id.name)
    }
}

impl<A: Allocator> Display for super::FunctionDef<'_, '_, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.decl)?;
        self.body.pretty(f, Level::new())
    }
}

#[inline]
fn sub_stmt_level<A: Allocator>(stmt: &super::Stmt<'_, '_, A>, level: Level) -> Level {
    if matches!(
        stmt,
        &super::Stmt::Block(_) | &super::Stmt::Case(..) | &super::Stmt::Labled(..)
    ) {
        level
    } else {
        level.next()
    }
}

#[inline]
fn block_stmt_level<A: Allocator>(stmt: &super::Stmt<'_, '_, A>, level: Level) -> Level {
    if matches!(stmt, &super::Stmt::Case(..) | &super::Stmt::Labled(..)) {
        level
    } else {
        level.next()
    }
}

impl<A: Allocator> Pretty for super::Stmt<'_, '_, A> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, level: Level) -> std::fmt::Result {
        let spaces = if matches!(self, super::Stmt::Labled(..)) {
            level.prev().to_spaces()
        } else {
            level.to_spaces()
        };
        write!(f, "{: >spaces$}", "")?;

        match self {
            Self::Block(stmts) => stmts.pretty(f, level),
            Self::Break(_) => write!(f, "break;"),
            Self::Case(_, expr, stmt) => {
                if let Some(expr) = expr {
                    writeln!(f, "case {expr}:")?;
                } else {
                    writeln!(f, "default:")?;
                }
                stmt.pretty(f, sub_stmt_level(stmt, level))
            }
            Self::Continue(_) => write!(f, "continue;"),
            Self::Decl(id, ty, init) => {
                write!(f, "{ty} {}", id.name)?;
                if let Some(init) = init {
                    write!(f, " = {init};")
                } else {
                    write!(f, ";")
                }
            }
            Self::Do(body, condition) => {
                writeln!(f, "do")?;
                body.pretty(f, sub_stmt_level(body, level))?;
                writeln!(f)?;
                write!(f, "{: >spaces$}while ({condition});", "")
            }
            Self::Expr(expr) => write!(f, "{expr};"),
            Self::For(for_stmt) => {
                write!(f, "for (")?;
                for_stmt
                    .init
                    .as_ref()
                    .map_or(&super::Stmt::Nil, |stmt| stmt.as_ref())
                    .pretty(f, Level::new())?;
                if let Some(condition) = &for_stmt.condition {
                    write!(f, " {condition}")?;
                }
                write!(f, "; ")?;
                if let Some(increment) = &for_stmt.increment {
                    write!(f, "{increment}")?;
                }
                writeln!(f, ")")?;
                for_stmt
                    .body
                    .pretty(f, sub_stmt_level(&for_stmt.body, level))
            }
            Self::FunctionDecl(decl) => write!(f, "{decl};"),
            Self::Goto(id) => write!(f, "goto {};", id.name),
            Self::If(condition, then_branch, else_branch) => {
                writeln!(f, "if ({})", condition)?;
                then_branch.pretty(f, sub_stmt_level(then_branch, level))?;
                if let Some(else_branch) = else_branch {
                    writeln!(f)?;
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
            Self::Labled(id, stmt) => {
                writeln!(f, "{}:", id.name)?;
                stmt.pretty(f, level)
            }
            Self::Nil => write!(f, ";"),
            Self::Return(expr) => write!(f, "return {expr};"),
            Self::Switch(expr, body) => {
                writeln!(f, "switch ({expr})")?;
                body.pretty(f, sub_stmt_level(body, level))
            }
            Self::While(condition, body) => {
                writeln!(f, "while ({condition})")?;
                body.pretty(f, sub_stmt_level(body, level))
            }
        }
    }
}

impl<A: Allocator> Pretty for Vec<super::Stmt<'_, '_, A>> {
    fn pretty(&self, f: &mut std::fmt::Formatter<'_>, level: Level) -> std::fmt::Result {
        let spaces = level.to_spaces();
        writeln!(f, "{{")?;
        for stmt in self {
            stmt.pretty(f, block_stmt_level(stmt, level))?;
            writeln!(f)?;
        }
        write!(f, "{: >spaces$}}}", "")
    }
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
            super::ExprTy::Var(id) => write!(f, "{id}"),
            super::ExprTy::Call(call) => {
                write!(f, "{}(", call.operand)?;
                if let Some(first) = call.args.first() {
                    write!(f, "{first}")?;
                    for param in &call.args[1..] {
                        write!(f, ", {param}")?;
                    }
                }
                write!(f, ")")
            }
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
