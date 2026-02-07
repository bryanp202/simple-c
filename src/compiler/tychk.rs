use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        asm::Label,
        ast::{self, AssignOp, BinaryOp, GlobalVar, Identifier, Item, UnaryOp},
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

struct GotoLabel<'src> {
    id: Identifier<'src>,
    label: Label,
    declared: bool,
}

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
    // Program data
    label_count: usize,
    // Function data
    goto_labels: Vec<GotoLabel<'src>>,
    local_count: usize,
}

impl<'src, 'a> TyChecker<'src, 'a> {
    pub fn new(ty_interner: &'a mut TyInterner<'src, 'a>, ast_arena: Alloc<'a>) -> Self {
        Self {
            poisoned_ty: ty_interner.intern(Ty::Poisoned),
            ast_arena,
            var_map: ScopeStack::new(),
            errors: Vec::new(),
            label_count: 0,
            goto_labels: Vec::new(),
            local_count: 0,
        }
    }

    pub fn check(
        mut self,
        src: &'src str,
        src_path: PathBuf,
        ast_program: Program<'src, 'a>,
    ) -> Result<TypedProgram<'src, 'a>, CompileError> {
        let mut globals = Vec::new();
        let mut functions = Vec::new();

        for item in ast_program.items {
            match item {
                Item::Fn(fun) => functions.push(self.function(fun)),
                Item::Var(global) => globals.push(self.global(global)),
            }
        }

        if self.errors.is_empty() {
            Ok(TypedProgram {
                labels: self.label_count,
                functions,
                globals,
            })
        } else {
            Err(CompileError::from_semantic_errors(
                src,
                src_path,
                self.errors,
            ))
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
    fn alloc_stmt(&self, stmt: TypedStmt<'src, 'a>) -> Box<TypedStmt<'src, 'a>, Alloc<'a>> {
        Box::new_in(stmt, self.ast_arena)
    }

    fn common_type(
        &self,
        lhs: Interned<'a, Ty<'src, 'a>>,
        rhs: Interned<'a, Ty<'src, 'a>>,
    ) -> Interned<'a, Ty<'src, 'a>> {
        if lhs == rhs { lhs } else { self.poisoned_ty }
    }

    fn get_or_make_label(&mut self, id: Identifier<'src>) -> Label {
        self.goto_labels
            .iter()
            .find(|goto| goto.id.name == id.name)
            .map(|goto| goto.label)
            .unwrap_or_else(|| {
                let label = Label(self.label_count);
                self.label_count += 1;
                self.goto_labels.push(GotoLabel {
                    id,
                    label,
                    declared: false,
                });
                label
            })
    }

    /// Make a new label for goto stmts
    ///
    /// Log err if label with same id already exists
    fn make_label(&mut self, id: Identifier<'src>) -> Label {
        let label = self
            .goto_labels
            .iter_mut()
            .find(|goto| goto.id.name == id.name);

        if let Some(goto_label) = label {
            if goto_label.declared {
                // log_err copied to appease borrow checker
                self.errors.push(SemanticErrorWithCtx {
                    ctx: goto_label.id.ctx.clone(),
                    err: SemanticError::DuplicateDecl,
                });
            }
            goto_label.declared = true;
            goto_label.label
        } else {
            let label = Label(self.label_count);
            self.label_count += 1;
            self.goto_labels.push(GotoLabel {
                id,
                label,
                declared: true,
            });
            label
        }
    }

    fn global(&mut self, global: GlobalVar<'src>) -> TypedGlobalVar<'src> {
        TypedGlobalVar { name: global.name }
    }

    fn function(&mut self, fun: Function<'src, 'a>) -> TypedFunction<'src, 'a> {
        self.reset_for_fn();

        let body = fun.body.into_iter().map(|stmt| self.stmt(stmt)).collect();

        // Check all goto labels resolved
        self.goto_labels
            .drain(..)
            .filter_map(|l| if l.declared { None } else { Some(l.id.ctx) })
            .for_each(|ctx| {
                self.errors.push(SemanticErrorWithCtx {
                    ctx,
                    err: SemanticError::UndeclaredVar,
                });
            });

        TypedFunction {
            name: fun.name,
            body,
            local_count: self.local_count,
        }
    }

    fn stmt(&mut self, stmt: Stmt<'src, 'a>) -> TypedStmt<'src, 'a> {
        match stmt {
            Stmt::Block(stmts) => {
                let old_scope_bottom = self.var_map.enter_scope();
                let stmts = stmts.into_iter().map(|stmt| self.stmt(stmt)).collect();
                self.var_map.exit_scope(old_scope_bottom);
                TypedStmt::Block(stmts)
            }
            Stmt::Expr(expr) => TypedStmt::Expr(self.expr(*expr)),
            Stmt::Decl(ident, ty, init) => {
                if self.var_map.in_scope(&ident.name) {
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

                let init = init.map(|expr| self.expr(*expr));
                TypedStmt::Decl(init)
            }
            Stmt::Goto(id) => {
                let label = self.get_or_make_label(id);
                TypedStmt::Goto(label)
            }
            Stmt::If(condition, then_branch, else_branch) => {
                let condition = self.expr(*condition);
                let then_branch = self.stmt(*then_branch);
                let else_branch = else_branch
                    .map(|stmt| self.stmt(*stmt))
                    .map(|stmt| self.alloc_stmt(stmt));
                TypedStmt::If(condition, self.alloc_stmt(then_branch), else_branch)
            }
            Stmt::Labled(id, stmt) => {
                let label = self.make_label(id);
                let stmt = self.stmt(*stmt);
                TypedStmt::Labled(label, self.alloc_stmt(stmt))
            }
            Stmt::Nil => TypedStmt::Nil,
            Stmt::Return(expr) => TypedStmt::Return(self.expr(*expr)),
        }
    }

    fn expr(&mut self, Expr { expr, ctx }: Expr<'src, 'a>) -> Box<TypedExpr<'src, 'a>, Alloc<'a>> {
        let typed = match expr {
            ExprTy::Ternary(cond, then_branch, else_branch) => {
                self.ternary(*cond, *then_branch, *else_branch)
            }
            ExprTy::Assign(op, lhs, rhs) => self.assign(op, *lhs, *rhs),
            ExprTy::Binary(op, lhs, rhs) => self.binary(op, *lhs, *rhs),
            ExprTy::Unary(op, operand) => self.unary(op, *operand),
            ExprTy::DecInc(op, operand) => self.decinc(op, *operand),
            ExprTy::Var(name) => self.variable(name, ctx),
            ExprTy::Constant(imm) => TypedExpr {
                expr: TypedExprTy::Constant(imm),
                ty: self.poisoned_ty,
            },
            ExprTy::Poisoned => TypedExpr {
                expr: TypedExprTy::Poisoned,
                ty: self.poisoned_ty,
            },
        };
        Box::new_in(typed, self.ast_arena)
    }

    fn ternary(
        &mut self,
        cond: Expr<'src, 'a>,
        then_branch: Expr<'src, 'a>,
        else_branch: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        let cond = self.expr(cond);
        let then_branch = self.expr(then_branch);
        let else_branch = self.expr(else_branch);
        let ty = self.common_type(then_branch.ty, else_branch.ty);

        TypedExpr {
            expr: TypedExprTy::Ternary(cond, then_branch, else_branch),
            ty,
        }
    }

    fn assign(
        &mut self,
        op: AssignOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        if is_lvalue(&lhs) {
            let lhs = self.expr(lhs);
            let rhs = self.expr(rhs);
            let ty = self.common_type(lhs.ty, rhs.ty);
            TypedExpr {
                expr: TypedExprTy::Assign(op, lhs, rhs),
                ty,
            }
        } else {
            self.error(lhs.ctx, SemanticError::InvalidLValue)
        }
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        let lhs = self.expr(lhs);
        let rhs = self.expr(rhs);
        let ty = self.common_type(lhs.ty, rhs.ty);
        TypedExpr {
            expr: TypedExprTy::Binary(op, lhs, rhs),
            ty,
        }
    }

    fn unary(&mut self, op: UnaryOp, operand: Expr<'src, 'a>) -> TypedExpr<'src, 'a> {
        if matches!(op, UnaryOp::Increment | UnaryOp::Decrement) && !is_lvalue(&operand) {
            self.error(operand.ctx, SemanticError::InvalidLValue)
        } else {
            let operand = self.expr(operand);
            let ty = operand.ty;
            TypedExpr {
                expr: TypedExprTy::Unary(op, operand),
                ty,
            }
        }
    }

    fn decinc(&mut self, op: UnaryOp, operand: Expr<'src, 'a>) -> TypedExpr<'src, 'a> {
        if is_lvalue(&operand) {
            let operand = self.expr(operand);
            let ty = operand.ty;
            TypedExpr {
                expr: TypedExprTy::DecInc(op, operand),
                ty,
            }
        } else {
            self.error(operand.ctx, SemanticError::InvalidLValue)
        }
    }

    fn variable(&mut self, name: Interned<'src, str>, ctx: Context) -> TypedExpr<'src, 'a> {
        match self.var_map.get(&name) {
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

fn is_lvalue(expr: &Expr<'_, '_>) -> bool {
    matches!(&expr.expr, &ExprTy::Var(_))
}
