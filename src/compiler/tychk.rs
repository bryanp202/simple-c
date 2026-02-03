use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, AssignOp, BinaryOp, GlobalVar, UnaryOp},
        error::{CompileError, Context, SemanticError, SemanticErrorWithCtx},
        ty::{ScopeStack, Ty, TyInterner},
    },
    intern::Interned,
};

type Alloc<'a> = &'a Arena<'static>;
type Program<'s, 'a> = ast::Program<'s, 'a, Alloc<'a>>;
type Function<'s, 'a> = ast::Function<'s, 'a, Alloc<'a>>;
type Stmt<'s, 'a> = ast::Stmt<'s, 'a, Alloc<'a>>;
type Expr<'s, 'a> = ast::Expr<'s, Alloc<'a>>;
type ExprTy<'s, 'a> = ast::ExprTy<'s, Alloc<'a>>;

type TypedProgram<'s, 'a> = ast::typed::Program<'s, 'a, Alloc<'a>>;
type TypedGlobalVar<'s> = ast::typed::GlobalVar<'s>;
type TypedFunction<'s, 'a> = ast::typed::Function<'s, 'a, Alloc<'a>>;
type TypedStmt<'s, 'a> = ast::typed::Stmt<'s, 'a, Alloc<'a>>;
type TypedExpr<'s, 'a> = ast::typed::Expr<'s, 'a, Alloc<'a>>;
type TypedExprTy<'s, 'a> = ast::typed::ExprTy<'s, 'a, Alloc<'a>>;

struct SymbolInfo<'src, 'a> {
    ty: Interned<'a, Ty<'src, 'a>>,
    scope: ScopeTy,
}

enum ScopeTy {
    Global,
    Local(usize), // Local # based on order of declaration
}

pub struct TyChecker<'src, 'a> {
    poisoned_ty: Interned<'a, Ty<'src, 'a>>,
    ast_arena: Alloc<'a>,
    var_map: ScopeStack<Interned<'src, str>, SymbolInfo<'src, 'a>>,
    errors: Vec<SemanticErrorWithCtx>,
    local_count: usize,
}

impl<'src, 'a> TyChecker<'src, 'a> {
    pub fn new(ty_interner: &'a mut TyInterner<'src, 'a>, ast_arena: Alloc<'a>) -> Self {
        Self {
            poisoned_ty: ty_interner.intern(Ty::Poisoned),
            ast_arena,
            var_map: ScopeStack::new(),
            errors: Vec::new(),
            local_count: 0,
        }
    }

    pub fn check(
        mut self,
        src: &'src str,
        src_path: PathBuf,
        ast_program: Program<'src, 'a>,
    ) -> Result<TypedProgram<'src, 'a>, CompileError> {
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
            Ok(TypedProgram { functions, globals })
        }
    }

    #[inline]
    fn reset_for_fn(&mut self) {
        self.local_count = 0;
    }

    #[inline]
    fn new_local(&mut self) -> usize {
        let local = self.local_count;
        self.local_count += 1;
        local
    }

    #[inline]
    fn log_err(&mut self, ctx: Context, err: SemanticError) {
        self.errors.push(SemanticErrorWithCtx { ctx, err });
    }

    #[inline]
    fn error(&mut self, ctx: Context, err: SemanticError) -> TypedExpr<'src, 'a> {
        self.log_err(ctx, err);
        TypedExpr {
            expr: TypedExprTy::Poisoned,
            ty: self.poisoned_ty,
        }
    }

    #[inline]
    fn alloc_expr(&self, expr: TypedExpr<'src, 'a>) -> Box<TypedExpr<'src, 'a>, Alloc<'a>> {
        Box::new_in(expr, self.ast_arena)
    }

    fn common_type(
        &self,
        lhs: Interned<'a, Ty<'src, 'a>>,
        rhs: Interned<'a, Ty<'src, 'a>>,
    ) -> Interned<'a, Ty<'src, 'a>> {
        if lhs == rhs { lhs } else { self.poisoned_ty }
    }
}

impl<'src, 'a> TyChecker<'src, 'a> {
    fn resolve_global(&mut self, global: GlobalVar<'src>) -> TypedGlobalVar<'src> {
        TypedGlobalVar { name: global.name }
    }

    fn resolve_fn(&mut self, fun: Function<'src, 'a>) -> TypedFunction<'src, 'a> {
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
        TypedFunction {
            name,
            body,
            local_count,
        }
    }

    fn resolve_stmt(&mut self, stmt: Stmt<'src, 'a>) -> TypedStmt<'src, 'a> {
        match stmt {
            Stmt::Block(stmts) => {
                let old_scope_bottom = self.var_map.enter_scope();
                let stmts = stmts
                    .into_iter()
                    .map(|stmt| self.resolve_stmt(stmt))
                    .collect();
                self.var_map.exit_scope(old_scope_bottom);
                return TypedStmt::Block(stmts);
            }
            Stmt::Expr(expr) => TypedStmt::Expr(self.resolve_expr(*expr)),
            Stmt::Decl(ident, ty, init) => {
                if self.var_map.in_scope(ident.name) {
                    self.log_err(ident.ctx, SemanticError::DuplicateDecl);
                } else {
                    let local = self.new_local();
                    self.var_map.push(
                        ident.name,
                        SymbolInfo {
                            ty,
                            scope: ScopeTy::Local(local),
                        },
                    );
                }

                let init = init.map(|expr| self.resolve_expr(*expr));
                TypedStmt::Decl(init)
            }
            Stmt::Nil => TypedStmt::Nil,
            Stmt::Return(expr) => TypedStmt::Return(self.resolve_expr(*expr)),
        }
    }

    fn resolve_expr(
        &mut self,
        Expr { expr, ctx }: Expr<'src, 'a>,
    ) -> Box<TypedExpr<'src, 'a>, Alloc<'a>> {
        let typed = match expr {
            ExprTy::Assign(op, lhs, rhs) => self.resolve_assign(op, *lhs, *rhs),
            ExprTy::Binary(op, lhs, rhs) => self.resolve_binary(op, *lhs, *rhs),
            ExprTy::Unary(op, operand) => self.resolve_unary(op, *operand),
            ExprTy::DecInc(op, operand) => self.resolve_decinc(op, *operand),
            ExprTy::Var(name) => self.resolve_var(name, ctx),
            ExprTy::Constant(imm) => TypedExpr {
                expr: TypedExprTy::Constant(imm),
                ty: self.poisoned_ty,
            },
            ExprTy::Poisoned => TypedExpr {
                expr: TypedExprTy::Poisoned,
                ty: self.poisoned_ty,
            },
        };
        self.alloc_expr(typed)
    }

    fn resolve_assign(
        &mut self,
        op: AssignOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        if is_lvalue(&lhs) {
            let lhs = self.resolve_expr(lhs);
            let rhs = self.resolve_expr(rhs);
            let ty = self.common_type(lhs.ty, rhs.ty);
            TypedExpr {
                expr: TypedExprTy::Assign(op, lhs, rhs),
                ty,
            }
        } else {
            self.error(lhs.ctx, SemanticError::InvalidLValue)
        }
    }

    fn resolve_binary(
        &mut self,
        op: BinaryOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        let lhs = self.resolve_expr(lhs);
        let rhs = self.resolve_expr(rhs);
        let ty = self.common_type(lhs.ty, rhs.ty);
        TypedExpr {
            expr: TypedExprTy::Binary(op, lhs, rhs),
            ty,
        }
    }

    fn resolve_unary(&mut self, op: UnaryOp, operand: Expr<'src, 'a>) -> TypedExpr<'src, 'a> {
        if matches!(op, UnaryOp::Increment | UnaryOp::Decrement) && !is_lvalue(&operand) {
            self.error(operand.ctx, SemanticError::InvalidLValue)
        } else {
            let operand = self.resolve_expr(operand);
            let ty = operand.ty;
            TypedExpr {
                expr: TypedExprTy::Unary(op, operand),
                ty,
            }
        }
    }

    fn resolve_decinc(&mut self, op: UnaryOp, operand: Expr<'src, 'a>) -> TypedExpr<'src, 'a> {
        if is_lvalue(&operand) {
            let operand = self.resolve_expr(operand);
            let ty = operand.ty;
            TypedExpr {
                expr: TypedExprTy::DecInc(op, operand),
                ty,
            }
        } else {
            self.error(operand.ctx, SemanticError::InvalidLValue)
        }
    }

    fn resolve_var(&mut self, name: Interned<'src, str>, ctx: Context) -> TypedExpr<'src, 'a> {
        match self.var_map.get(name) {
            None => self.error(ctx, SemanticError::UndeclaredVar),
            Some(&SymbolInfo {
                ty,
                scope: ScopeTy::Global,
            }) => TypedExpr {
                expr: TypedExprTy::Global(name),
                ty,
            },
            Some(&SymbolInfo {
                ty,
                scope: ScopeTy::Local(id),
            }) => TypedExpr {
                expr: TypedExprTy::Local(id),
                ty,
            },
        }
    }
}

fn is_lvalue<'src, 'a>(expr: &Expr<'src, 'a>) -> bool {
    matches!(&expr.expr, &ExprTy::Var(_))
}
