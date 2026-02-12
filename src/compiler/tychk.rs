use std::{collections::HashMap, path::PathBuf};

use crate::{
    arena::Arena,
    compiler::{
        asm::Label,
        ast::{
            self, AssignOp, BinaryOp, FunctionDecl, GlobalVar, Identifier, Item, UnaryOp,
            typed::{ForStmt, SwitchCase, SwitchStmt},
        },
        error::{CompileError, Context, SemanticError, SemanticErrorWithCtx},
        ty::{ScopeStack, Ty, TyInterner},
    },
    intern::Interned,
};

type Alloc<'a> = &'a Arena<'static>;
type Program<'s, 'a> = ast::Program<'s, 'a, Alloc<'a>>;
type FunctionDef<'s, 'a> = ast::FunctionDef<'s, 'a, Alloc<'a>>;
type Stmt<'s, 'a> = ast::Stmt<'s, 'a, Alloc<'a>>;
type Expr<'s, 'a> = ast::Expr<'s, Alloc<'a>>;
type ExprTy<'s, 'a> = ast::ExprTy<'s, Alloc<'a>>;

type TypedProgram<'s, 'a> = ast::typed::Program<'s, 'a, Alloc<'a>>;
type TypedGlobalVar<'s> = ast::typed::GlobalVar<'s>;
type TypedFunction<'s, 'a> = ast::typed::Function<'s, 'a, Alloc<'a>>;
type TypedStmt<'s, 'a> = ast::typed::Stmt<'s, 'a, Alloc<'a>>;
type TypedExpr<'s, 'a> = ast::typed::Expr<'s, 'a, Alloc<'a>>;
type TypedExprTy<'s, 'a> = ast::typed::ExprTy<'s, 'a, Alloc<'a>>;

type SwitchData<'s, 'a> = (Vec<SwitchCase>, Option<Label>);
struct SymbolData<'s, 'a> {
    is_defined: bool,
    ty: Interned<'a, Ty<'s, 'a>>,
}

struct GotoLabel<'src> {
    id: Identifier<'src>,
    label: Label,
    declared: bool,
}

struct IdInfo<'src, 'a> {
    ty: Interned<'a, Ty<'src, 'a>>,
    scope: ScopeTy,
}

#[derive(PartialEq, Eq)]
enum ScopeTy {
    External,
    Internal,
    Local(usize), // Local # based on order of declaration
}

pub struct TyChecker<'src, 'a> {
    poisoned_ty: Interned<'a, Ty<'src, 'a>>,
    ast_arena: Alloc<'a>,
    errors: Vec<SemanticErrorWithCtx>,
    // Program data
    id_map: ScopeStack<Interned<'src, str>, IdInfo<'src, 'a>>,
    symbol_table: HashMap<Interned<'src, str>, SymbolData<'src, 'a>>,
    label_count: usize,
    // Function data
    goto_labels: Vec<GotoLabel<'src>>,
    local_count: usize,
    loop_depth: usize,
    switch_cases: Option<SwitchData<'src, 'a>>,
}

impl<'src, 'a> TyChecker<'src, 'a> {
    pub fn new(ty_interner: &'a mut TyInterner<'src, 'a>, ast_arena: Alloc<'a>) -> Self {
        Self {
            poisoned_ty: ty_interner.intern(Ty::Poisoned),
            ast_arena,
            errors: Vec::new(),
            id_map: ScopeStack::new(),
            symbol_table: HashMap::new(),
            label_count: 0,
            goto_labels: Vec::new(),
            local_count: 0,
            loop_depth: 0,
            switch_cases: None,
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
                Item::FnDecl(decl) => self.function_decl(decl),
                Item::FnDef(def) => functions.push(self.function_def(def)),
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

    #[inline]
    fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    #[inline]
    fn exit_loop(&mut self) {
        self.loop_depth -= 1;
    }

    #[inline]
    fn enter_switch(&mut self) -> Option<SwitchData<'src, 'a>> {
        self.switch_cases.replace((Vec::new(), None))
    }

    #[inline]
    fn exit_switch(
        &mut self,
        old_switch_data: Option<SwitchData<'src, 'a>>,
    ) -> SwitchData<'src, 'a> {
        let old = std::mem::replace(&mut self.switch_cases, old_switch_data);
        old.expect("exit_switch must be called after enter_switch")
    }

    #[inline]
    fn can_break(&self) -> bool {
        self.loop_depth > 0 || self.switch_cases.is_some()
    }

    #[inline]
    fn can_continue(&mut self) -> bool {
        self.loop_depth > 0
    }

    fn declare_id(&mut self, id: Identifier<'src>, id_info: IdInfo<'src, 'a>) {
        if let Some(old_id_info) = self.id_map.get_in_scope(&id.name)
            && old_id_info.scope != ScopeTy::External
            && id_info.scope != ScopeTy::External
        {
            self.error(id.ctx, SemanticError::DuplicateDecl);
        } else {
            self.id_map.push(id.name, id_info);
        }
    }

    fn common_type(
        &mut self,
        ctx: Context,
        lhs: Interned<'a, Ty<'src, 'a>>,
        rhs: Interned<'a, Ty<'src, 'a>>,
    ) -> Interned<'a, Ty<'src, 'a>> {
        if lhs == rhs {
            lhs
        } else if lhs.eq_or_poison(&rhs) {
            self.poisoned_ty
        } else {
            self.log_err(ctx, SemanticError::TypeMismatch);
            self.poisoned_ty
        }
    }

    fn common_type_static(
        &mut self,
        ctx: Context,
        static_ty: Ty<'src, 'a>,
        other: Interned<'a, Ty<'src, 'a>>,
    ) -> Interned<'a, Ty<'src, 'a>> {
        if static_ty == *other {
            other
        } else if static_ty.eq_or_poison(&other) {
            self.poisoned_ty
        } else {
            self.log_err(ctx, SemanticError::TypeMismatch);
            self.poisoned_ty
        }
    }

    fn new_label(&mut self) -> Label {
        let label = Label(self.label_count);
        self.label_count += 1;
        label
    }

    fn get_or_make_goto_label(&mut self, id: Identifier<'src>) -> Label {
        self.goto_labels
            .iter()
            .find(|goto| goto.id.name == id.name)
            .map(|goto| goto.label)
            .unwrap_or_else(|| {
                let label = self.new_label();
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
    fn make_goto_label(&mut self, id: Identifier<'src>) -> Label {
        let label = self
            .goto_labels
            .iter_mut()
            .find(|goto| goto.id.name == id.name);

        if let Some(goto_label) = label {
            if goto_label.declared {
                // log_err copied to appease borrow checker
                self.errors.push(SemanticErrorWithCtx {
                    ctx: id.ctx.clone(),
                    err: SemanticError::DuplicateDecl,
                });
            }
            goto_label.declared = true;
            goto_label.label
        } else {
            let label = self.new_label();
            self.goto_labels.push(GotoLabel {
                id,
                label,
                declared: true,
            });
            label
        }
    }

    fn global(&mut self, global: GlobalVar<'src>) -> TypedGlobalVar<'src> {
        TypedGlobalVar {
            name: global.id.name,
        }
    }

    fn function_decl(&mut self, decl: FunctionDecl<'src, 'a>) {
        let symbol = self.symbol_table.entry(decl.id.name).or_insert(SymbolData {
            is_defined: false,
            ty: decl.ty,
        });

        if symbol.ty != decl.ty {
            self.log_err(decl.id.ctx.clone(), SemanticError::TypeMismatch);
        } else {
            let id_info = IdInfo {
                ty: decl.ty,
                scope: ScopeTy::External,
            };
            self.declare_id(decl.id, id_info);
        }

        let old_scope = self.id_map.enter_scope();
        for param in decl.param_names {
            self.declare_id(
                param,
                IdInfo {
                    ty: self.poisoned_ty,
                    scope: ScopeTy::Local(0),
                },
            );
        }
        self.id_map.exit_scope(old_scope);
    }

    fn function_def(&mut self, fun: FunctionDef<'src, 'a>) -> TypedFunction<'src, 'a> {
        self.reset_for_fn();

        let symbol = self
            .symbol_table
            .entry(fun.decl.id.name)
            .or_insert(SymbolData {
                is_defined: false,
                ty: fun.decl.ty,
            });
        match (symbol.is_defined, symbol.ty == fun.decl.ty) {
            (true, false) => {
                self.log_err(fun.decl.id.ctx.clone(), SemanticError::DuplicateDef);
                self.log_err(fun.decl.id.ctx.clone(), SemanticError::TypeMismatch);
            }
            (true, _) => self.log_err(fun.decl.id.ctx.clone(), SemanticError::DuplicateDef),
            (_, false) => self.log_err(fun.decl.id.ctx.clone(), SemanticError::TypeMismatch),
            _ => symbol.is_defined = true,
        }

        let old_scope = self.id_map.enter_scope();
        let Ty::Function { params, .. } = &*fun.decl.ty else {
            unreachable!("function decl had non-fn type")
        };
        for (id, ty) in fun.decl.param_names.into_iter().zip(params) {
            self.decl_local(id, *ty);
        }
        let body = fun.body.into_iter().map(|stmt| self.stmt(stmt)).collect();
        self.id_map.exit_scope(old_scope);

        let name = fun.decl.id.name;
        self.declare_id(
            fun.decl.id,
            IdInfo {
                ty: fun.decl.ty,
                scope: ScopeTy::External,
            },
        );

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
            name,
            body,
            local_count: self.local_count,
        }
    }

    fn decl_local(&mut self, id: Identifier<'src>, ty: Interned<'a, Ty<'src, 'a>>) {
        let local = self.new_local();
        self.declare_id(
            id,
            IdInfo {
                ty,
                scope: ScopeTy::Local(local),
            },
        );
    }

    fn stmt(&mut self, stmt: Stmt<'src, 'a>) -> TypedStmt<'src, 'a> {
        match stmt {
            Stmt::Block(stmts) => {
                let old_scope = self.id_map.enter_scope();
                let stmts = stmts.into_iter().map(|stmt| self.stmt(stmt)).collect();
                self.id_map.exit_scope(old_scope);
                TypedStmt::Block(stmts)
            }
            Stmt::Break(ctx) => {
                if !self.can_break() {
                    self.log_err(ctx, SemanticError::InvalidBreak);
                }
                TypedStmt::Break
            }
            Stmt::Case(ctx, Some(expr), body) => self.switch_case(ctx, *expr, *body),
            Stmt::Case(ctx, None, body) => self.switch_default(ctx, *body),
            Stmt::Continue(ctx) => {
                if !self.can_continue() {
                    self.log_err(ctx, SemanticError::InvalidContinue);
                }
                TypedStmt::Continue
            }
            Stmt::Do(body, condition) => {
                self.enter_loop();
                let body = self.stmt(*body);
                let condition = self.expr(*condition);
                self.exit_loop();
                TypedStmt::Do(self.alloc_stmt(body), condition)
            }
            Stmt::Expr(expr) => TypedStmt::Expr(self.expr(*expr)),
            Stmt::Decl(ident, ty, init) => {
                self.decl_local(ident, ty);

                let init = init.map(|expr| self.expr(*expr));
                TypedStmt::Decl(init)
            }
            Stmt::For(for_stmt) => {
                self.enter_loop();
                let old_scope = self.id_map.enter_scope();

                let init = for_stmt
                    .init
                    .map(|stmt| self.stmt(*stmt))
                    .map(|stmt| self.alloc_stmt(stmt));
                let condition = for_stmt.condition.map(|expr| self.expr(*expr));
                let increment = for_stmt.increment.map(|expr| self.expr(*expr));

                let body = self.stmt(*for_stmt.body);
                let for_stmt = ForStmt {
                    init,
                    condition,
                    increment,
                    body: self.alloc_stmt(body),
                };

                self.id_map.exit_scope(old_scope);
                self.exit_loop();
                TypedStmt::For(Box::new_in(for_stmt, self.ast_arena))
            }
            Stmt::FunctionDecl(decl) => {
                self.function_decl(*decl);
                TypedStmt::Nil
            }
            Stmt::Goto(id) => {
                let label = self.get_or_make_goto_label(id);
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
                let label = self.make_goto_label(id);
                let stmt = self.stmt(*stmt);
                TypedStmt::Labled(label, self.alloc_stmt(stmt))
            }
            Stmt::Nil => TypedStmt::Nil,
            Stmt::Return(expr) => TypedStmt::Return(self.expr(*expr)),
            Stmt::Switch(expr, body) => self.switch_stmt(*expr, *body),
            Stmt::While(condition, body) => {
                self.enter_loop();
                let condition = self.expr(*condition);
                let body = self.stmt(*body);
                self.exit_loop();
                TypedStmt::While(condition, self.alloc_stmt(body))
            }
        }
    }

    fn switch_stmt(&mut self, expr: Expr<'src, 'a>, body: Stmt<'src, 'a>) -> TypedStmt<'src, 'a> {
        let old_switch_data = self.enter_switch();
        let expr = self.expr(expr);

        let body = self.stmt(body);

        let (cases, default) = self.exit_switch(old_switch_data);
        let switch_stmt = SwitchStmt {
            expr,
            cases,
            default,
            body: self.alloc_stmt(body),
        };
        TypedStmt::Switch(Box::new_in(switch_stmt, self.ast_arena))
    }

    fn switch_case(
        &mut self,
        ctx: Context,
        expr: Expr<'src, 'a>,
        body: Stmt<'src, 'a>,
    ) -> TypedStmt<'src, 'a> {
        let label = self.new_label();

        if let Some(switch_cases) = &mut self.switch_cases {
            let ctx = expr.ctx.clone();
            if let ExprTy::Constant(imm) = expr.expr {
                let duplicate = switch_cases
                    .0
                    .iter()
                    .map(|case| case.val)
                    .any(|val| val == imm);
                if duplicate {
                    self.log_err(ctx, SemanticError::DuplicateCase);
                } else {
                    switch_cases.0.push(SwitchCase { val: imm, label });
                }
            } else {
                self.log_err(ctx, SemanticError::InvalidCaseExpr);
            };
        } else {
            self.log_err(ctx, SemanticError::InvalidCase);
        }

        let stmt = self.stmt(body);
        TypedStmt::Labled(label, self.alloc_stmt(stmt))
    }

    fn switch_default(&mut self, ctx: Context, body: Stmt<'src, 'a>) -> TypedStmt<'src, 'a> {
        let label = self.new_label();

        if let Some(switch_cases) = &mut self.switch_cases {
            if switch_cases.1.is_none() {
                switch_cases.1 = Some(label);
            } else {
                self.log_err(ctx, SemanticError::MultipleDefaultCases);
            }
        } else {
            self.log_err(ctx, SemanticError::InvalidCase);
        }

        let stmt = self.stmt(body);
        TypedStmt::Labled(label, self.alloc_stmt(stmt))
    }

    fn expr(&mut self, Expr { expr, ctx }: Expr<'src, 'a>) -> Box<TypedExpr<'src, 'a>, Alloc<'a>> {
        let typed = match expr {
            ExprTy::Ternary(cond, then_branch, else_branch) => {
                self.ternary(ctx, *cond, *then_branch, *else_branch)
            }
            ExprTy::Assign(op, lhs, rhs) => self.assign(ctx, op, *lhs, *rhs),
            ExprTy::Binary(op, lhs, rhs) => self.binary(ctx, op, *lhs, *rhs),
            ExprTy::Unary(op, operand) => self.unary(op, *operand),
            ExprTy::DecInc(op, operand) => self.decinc(op, *operand),
            ExprTy::Var(name) => self.variable(ctx, name),
            ExprTy::Constant(imm) => TypedExpr {
                expr: TypedExprTy::Constant(imm),
                ty: self.poisoned_ty,
            },
            ExprTy::Poisoned => TypedExpr {
                expr: TypedExprTy::Poisoned,
                ty: self.poisoned_ty,
            },
            ExprTy::Call(call_expr) => self.call_expr(ctx, *call_expr),
        };
        Box::new_in(typed, self.ast_arena)
    }

    fn ternary(
        &mut self,
        ctx: Context,
        cond: Expr<'src, 'a>,
        then_branch: Expr<'src, 'a>,
        else_branch: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        let cond = self.expr(cond);
        let then_branch = self.expr(then_branch);
        let else_branch = self.expr(else_branch);
        let ty = self.common_type(ctx, then_branch.ty, else_branch.ty);

        TypedExpr {
            expr: TypedExprTy::Ternary(cond, then_branch, else_branch),
            ty,
        }
    }

    fn assign(
        &mut self,
        ctx: Context,
        op: AssignOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        if is_lvalue(&lhs) {
            let lhs = self.expr(lhs);
            let rhs = self.expr(rhs);
            let ty = self.common_type(ctx, lhs.ty, rhs.ty);
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
        ctx: Context,
        op: BinaryOp,
        lhs: Expr<'src, 'a>,
        rhs: Expr<'src, 'a>,
    ) -> TypedExpr<'src, 'a> {
        let lhs = self.expr(lhs);
        let rhs = self.expr(rhs);
        let ty = self.common_type(ctx, lhs.ty, rhs.ty);
        TypedExpr {
            expr: TypedExprTy::Binary(op, lhs, rhs),
            ty,
        }
    }

    fn unary(&mut self, op: UnaryOp, operand: Expr<'src, 'a>) -> TypedExpr<'src, 'a> {
        if matches!(op, UnaryOp::Increment | UnaryOp::Decrement) && !is_lvalue(&operand) {
            self.error(operand.ctx, SemanticError::InvalidLValue)
        } else {
            let ctx = operand.ctx.clone();
            let operand = self.expr(operand);
            let ty = self.common_type_static(ctx, Ty::Int, operand.ty);
            TypedExpr {
                expr: TypedExprTy::Unary(op, operand),
                ty,
            }
        }
    }

    fn decinc(&mut self, op: UnaryOp, operand: Expr<'src, 'a>) -> TypedExpr<'src, 'a> {
        if is_lvalue(&operand) {
            let ctx = operand.ctx.clone();
            let operand = self.expr(operand);
            let ty = self.common_type_static(ctx, Ty::Int, operand.ty);
            TypedExpr {
                expr: TypedExprTy::DecInc(op, operand),
                ty,
            }
        } else {
            self.error(operand.ctx, SemanticError::InvalidLValue)
        }
    }

    fn call_expr(
        &mut self,
        ctx: Context,
        call_expr: ast::CallExpr<'src, Alloc<'a>>,
    ) -> TypedExpr<'src, 'a> {
        let operand_ctx = call_expr.operand.ctx.clone();
        let operand = self.expr(*call_expr.operand);
        let ty = operand.ty;

        let Ty::Function { ret, params } = &*ty else {
            return self.error(operand_ctx, SemanticError::ExpectedFunctionType);
        };

        let mut args = Vec::with_capacity_in(call_expr.args.len(), self.ast_arena);

        if call_expr.args.len() != params.len() {
            self.log_err(ctx, SemanticError::InvalidArgCount);
        }

        for (arg, param) in call_expr.args.into_iter().zip(params) {
            let ctx = arg.ctx.clone();
            let typed_arg = self.expr(*arg);
            if !typed_arg.ty.eq_or_poison(param) {
                self.log_err(ctx, SemanticError::TypeMismatch);
            }
            args.push(typed_arg);
        }

        let call_expr = ast::typed::CallExpr { operand, args };

        TypedExpr {
            expr: TypedExprTy::Call(Box::new_in(call_expr, self.ast_arena)),
            ty: *ret,
        }
    }

    fn variable(&mut self, ctx: Context, name: Interned<'src, str>) -> TypedExpr<'src, 'a> {
        match self.id_map.get(&name) {
            None => self.error(ctx, SemanticError::UndeclaredVar),
            Some(&IdInfo {
                ty,
                scope: ScopeTy::Internal | ScopeTy::External,
            }) => TypedExpr {
                expr: TypedExprTy::Global(name),
                ty,
            },
            Some(&IdInfo {
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
