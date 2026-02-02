use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, GlobalVar, UnaryOp},
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
        ast_program: Program<'src, 'a>,
    ) -> Result<Program<'src, 'a>, CompileError> {
        let functions = ast_program
            .functions
            .into_iter()
            .map(|fun| self.resolve_fn(fun))
            .collect();

        let globals = ast_program
            .globals
            .into_iter()
            .map(|global| self.resolve_global(global))
            .collect();

        if !self.errors.is_empty() {
            Err(CompileError::from_semantic_errors(
                src,
                src_path,
                self.errors,
            ))
        } else {
            Ok(Program { functions, globals })
        }
    }

    fn resolve_global(&mut self, global: GlobalVar<'src>) -> GlobalVar<'src> {
        global
    }

    fn resolve_fn(&mut self, fun: Function<'src, 'a>) -> Function<'src, 'a> {
        let Function {
            name,
            body,
            local_count,
        } = fun;
        self.reset_for_fn();

        let body = body
            .into_iter()
            .map(|stmt| self.resolve_stmt(stmt))
            .collect();
        Function {
            name,
            body,
            local_count,
        }
    }

    fn resolve_stmt(&mut self, stmt: Stmt<'src, 'a>) -> Stmt<'src, 'a> {
        match stmt {
            Stmt::Block(stmts) => {
                let old_scope_bottom = self.var_map.enter_scope();
                let stmts = stmts
                    .into_iter()
                    .map(|stmt| self.resolve_stmt(stmt))
                    .collect();
                self.var_map.exit_scope(old_scope_bottom);
                return Stmt::Block(stmts);
            }
            Stmt::Expr(expr) => Stmt::Expr(self.resolve_expr(expr)),
            Stmt::Decl(name, ty, init) => {
                if self.var_map.in_scope(name) {
                    self.log_err(Self::error(SemanticError::DuplicateDecl));
                } else {
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
                }

                let init = init.map(|expr| self.resolve_expr(expr));
                Stmt::Decl(name, ty, init)
            }
            Stmt::Nil => Stmt::Nil,
            Stmt::Return(expr) => Stmt::Return(self.resolve_expr(expr)),
        }
    }

    fn resolve_expr(
        &mut self,
        mut expr: Box<Expr<'src, 'a>, Alloc<'a>>,
    ) -> Box<Expr<'src, 'a>, Alloc<'a>> {
        *expr = match *expr {
            Expr::Assign(_, lhs, _) if !Self::is_lvalue(&lhs) => {
                self.log_err(Self::error(SemanticError::InvalidLValue));
                Expr::Poisoned
            }
            Expr::Assign(op, lhs, rhs) => {
                Expr::Assign(op, self.resolve_expr(lhs), self.resolve_expr(rhs))
            }
            Expr::Binary(op, lhs, rhs) => {
                Expr::Binary(op, self.resolve_expr(lhs), self.resolve_expr(rhs))
            }
            Expr::Unary(op, operand) => match op {
                UnaryOp::Decrement | UnaryOp::Increment if !Self::is_lvalue(&operand) => {
                    self.log_err(Self::error(SemanticError::InvalidLValue));
                    Expr::Poisoned
                }
                _ => Expr::Unary(op, self.resolve_expr(operand)),
            },
            Expr::DecInc(_, ref operand) if !Self::is_lvalue(operand) => {
                self.log_err(Self::error(SemanticError::InvalidLValue));
                Expr::Poisoned
            }
            Expr::DecInc(op, operand) => Expr::DecInc(op, self.resolve_expr(operand)),
            Expr::Var(name) => match self.var_map.get(name) {
                None => {
                    self.log_err(Self::error(SemanticError::UndeclaredVar));
                    Expr::Poisoned
                }
                Some(SymbolInfo {
                    attributes:
                        Attributes {
                            scope: ScopeTy::Global,
                        },
                    ..
                }) => Expr::Global(name),
                Some(SymbolInfo {
                    attributes:
                        Attributes {
                            scope: ScopeTy::Local(id),
                        },
                    ..
                }) => Expr::Local(*id),
            },
            Expr::Constant(_) | Expr::Global(_) | Expr::Local(_) | Expr::Poisoned => *expr,
        };
        expr
    }

    fn is_lvalue(expr: &Expr<'src, 'a>) -> bool {
        matches!(expr, &Expr::Var(_))
    }
}
