use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, GlobalVar},
        error::{CompileError, Context, SemanticError, SemanticErrorWithCtx},
        ty::{ScopeStack, Ty},
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
        stmt: Stmt<'src, 'a>,
        stmts: &mut Vec<Stmt<'src, 'a>>,
    ) -> Result<(), SemanticErrorWithCtx> {
        match stmt {
            Stmt::Block(sub_stmts) => {
                let types_old_top = self.var_map.enter_scope();
                for sub_stmt in sub_stmts {
                    self.resolve_stmt(sub_stmt, stmts)?;
                }
                self.var_map.exit_scope(types_old_top);
            }
            Stmt::Expr(expr) => {
                stmts.push(Stmt::Expr(self.resolve_expr(expr)?));
            }
            Stmt::Decl(name, ty, init) => {
                if let Some(_) = self.var_map.get(name) {
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

                let init = if let Some(expr) = init {
                    Some(self.resolve_expr(expr)?)
                } else {
                    init
                };
                stmts.push(Stmt::Decl(name, ty, init));
            }
            Stmt::Nil => {}
            Stmt::Return(expr) => stmts.push(Stmt::Return(self.resolve_expr(expr)?)),
        }

        Ok(())
    }

    fn resolve_expr(
        &mut self,
        mut expr: Box<Expr<'src, 'a>, Alloc<'a>>,
    ) -> Result<Box<Expr<'src, 'a>, Alloc<'a>>, SemanticErrorWithCtx> {
        match *expr {
            Expr::Assign(_, lhs, _) if !matches!(*lhs, Expr::Var(_)) => {
                Err(Self::error(SemanticError::InvalidLValue))
            }
            Expr::Assign(op, lhs, rhs) => {
                *expr = Expr::Assign(op, self.resolve_expr(lhs)?, self.resolve_expr(rhs)?);
                Ok(expr)
            }
            Expr::Binary(op, lhs, rhs) => {
                *expr = Expr::Binary(op, self.resolve_expr(lhs)?, self.resolve_expr(rhs)?);
                Ok(expr)
            }
            Expr::Unary(op, operand) => {
                *expr = Expr::Unary(op, self.resolve_expr(operand)?);
                Ok(expr)
            }
            Expr::Var(name) => match self.var_map.get(name) {
                None => Err(Self::error(SemanticError::UndeclaredVar)),
                Some(SymbolInfo {
                    attributes:
                        Attributes {
                            scope: ScopeTy::Global,
                        },
                    ..
                }) => {
                    *expr = Expr::Global(name);
                    Ok(expr)
                }
                Some(SymbolInfo {
                    attributes:
                        Attributes {
                            scope: ScopeTy::Local(id),
                        },
                    ..
                }) => {
                    *expr = Expr::Local(*id);
                    Ok(expr)
                }
            },
            Expr::Constant(_) | Expr::Global(_) | Expr::Local(_) => Ok(expr),
        }
    }
}
