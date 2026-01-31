use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, GlobalVar, UnaryOp}, error::{CompileError, Context, SemanticError, SemanticErrorWithCtx}, ty::{ScopeStack, Ty}
    },
    intern::Interned,
};

type Alloc<'a> = &'a Arena<'static>;
type Program<'s, 'a> = ast::Program<'s, 'a, Alloc<'a>>;
type Function<'s, 'a> = ast::Function<'s, 'a, Alloc<'a>>;
type Stmt<'s, 'a> = ast::Stmt<'s, 'a, Alloc<'a>>;
type Expr<'s, 'a> = ast::Expr<'s, Alloc<'a>>;

struct SymbolInfo<'src, 'a> {
    ty: Interned<'a, Ty<'src, 'a>>,
    attributes: Attributes,
}

struct Attributes {
    scope: ScopeTy,
}

enum ScopeTy {
    Global,
    Local(usize), // Local # based on order of declaration
}

pub struct TyChecker<'src, 'a> {
    var_map: ScopeStack<Interned<'src, str>, SymbolInfo<'src, 'a>>,
    errors: Vec<SemanticErrorWithCtx>,
    local_count: usize,
}

impl<'src, 'a> TyChecker<'src, 'a> {
    pub fn new() -> Self {
        Self {
            var_map: ScopeStack::new(),
            errors: Vec::new(),
            local_count: 0,
        }
    }

    fn reset_for_fn(&mut self) {
        self.local_count = 0;
    }

    fn new_local(&mut self) -> usize {
        let local = self.local_count;
        self.local_count += 1;
        local
    }

    fn error(err: SemanticError) -> SemanticErrorWithCtx {
        SemanticErrorWithCtx {
            ctx: Context::from(0..1),
            err,
        }
    }

    fn log_err(&mut self, err: SemanticErrorWithCtx) {
        self.errors.push(err);
    }

    pub fn check(
        mut self,
        src: &'src str,
        src_path: PathBuf,
        mut ast_program: Program<'src, 'a>,
    ) -> Result<Program<'src, 'a>, CompileError> {
        for function in &mut ast_program.functions {
            self.reset_for_fn();
            self.resolve_fn(function);
        }

        for global in &mut ast_program.globals {
            self.resolve_global(global);
        }

        if !self.errors.is_empty() {
            Err(CompileError::from_semantic_errors(
                src,
                src_path,
                self.errors,
            ))
        } else {
            Ok(ast_program)
        }
    }

    fn resolve_global(&mut self, global: &mut GlobalVar<'src>) {}

    fn resolve_fn(&mut self, fun: &mut Function<'src, 'a>) {
        let body = std::mem::take(&mut fun.body);
        let mut new_body = Vec::new();
        for stmt in body {
            match self.resolve_stmt(stmt, &mut new_body) {
                Ok(()) => {}
                Err(err) => self.log_err(err),
            }
        }

        std::mem::swap(&mut fun.body, &mut new_body);
    }

    fn resolve_stmt(
        &mut self,
        mut stmt: Stmt<'src, 'a>,
        stmts: &mut Vec<Stmt<'src, 'a>>,
    ) -> Result<(), SemanticErrorWithCtx> {
        match stmt {
            Stmt::Block(sub_stmts) => {
                let old_scope_bottom = self.var_map.enter_scope();
                for sub_stmt in sub_stmts {
                    self.resolve_stmt(sub_stmt, stmts)?;
                }
                self.var_map.exit_scope(old_scope_bottom);
                return Ok(());
            }
            Stmt::Expr(ref mut expr) => self.resolve_expr(expr)?,
            Stmt::Decl(name, ty, ref mut init) => {
                if self.var_map.in_scope(name) {
                    return Err(Self::error(SemanticError::DuplicateDecl));
                }

                let local = self.new_local();
                self.var_map.push(
                    name,
                    SymbolInfo {
                        ty,
                        attributes: Attributes {
                            scope: ScopeTy::Local(local),
                        },
                    },
                );

                if let Some(expr) = init {
                    self.resolve_expr(expr)?;
                }
            }
            Stmt::Nil => {}
            Stmt::Return(ref mut expr) => self.resolve_expr(expr)?,
        }

        stmts.push(stmt);

        Ok(())
    }

    fn resolve_expr(
        &mut self,
        expr: &mut Expr<'src, 'a>,
    ) -> Result<(), SemanticErrorWithCtx> {
        match expr {
            Expr::Assign(_, lhs, _) if !Self::is_lvalue(&lhs) => {
                Err(Self::error(SemanticError::InvalidLValue))
            }
            Expr::Assign(_, lhs, rhs) => self.resolve_exprs(lhs, rhs),
            Expr::Binary(_, lhs, rhs) => self.resolve_exprs(lhs, rhs),
            Expr::Unary(op, operand) => {
                match op {
                    UnaryOp::Decrement | UnaryOp::Increment if !Self::is_lvalue(&operand) => Err(Self::error(SemanticError::InvalidLValue)),
                    _ => self.resolve_expr(operand)
                }
            }
            Expr::DecInc(_, operand) if !Self::is_lvalue(operand) => {
                Err(Self::error(SemanticError::InvalidLValue))
            }
            Expr::DecInc(_, operand) => self.resolve_expr(operand),
            Expr::Var(name) => match self.var_map.get(*name) {
                None => Err(Self::error(SemanticError::UndeclaredVar)),
                Some(SymbolInfo {
                    attributes:
                        Attributes {
                            scope: ScopeTy::Global,
                        },
                    ..
                }) => {
                    *expr = Expr::Global(*name);
                    Ok(())
                }
                Some(SymbolInfo {
                    attributes:
                        Attributes {
                            scope: ScopeTy::Local(id),
                        },
                    ..
                }) => {
                    *expr = Expr::Local(*id);
                    Ok(())
                }
            },
            Expr::Constant(_) | Expr::Global(_) | Expr::Local(_) => Ok(()),
        }
    }

    fn resolve_exprs(&mut self, lhs: &mut Expr<'src, 'a>, rhs: &mut Expr<'src, 'a>) -> Result<(), SemanticErrorWithCtx> {
        self.resolve_expr(lhs)?;
        self.resolve_expr(rhs)
    }

    fn is_lvalue(expr: &Expr<'src, 'a>) -> bool {
        matches!(expr, &Expr::Var(_))
    }
}