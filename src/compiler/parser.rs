use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, AssignOp, BinaryOp, Function, GlobalVar, Identifier, UnaryOp},
        error::{CompileError, Context, SyntaxError, SyntaxErrorWithCtx},
        lexer::Lexer,
        token::{Precedence, Token, TokenTy},
        ty::{Ty, TyInterner, TyStack},
    },
    intern::{Interned, Interner},
};

type Alloc<'a> = &'a Arena<'static>;
type Program<'s, 'a> = ast::Program<'s, 'a, Alloc<'a>>;
type Item<'s, 'a> = ast::Item<'s, 'a, Alloc<'a>>;
type Stmt<'s, 'a> = ast::Stmt<'s, 'a, Alloc<'a>>;
type Expr<'s, 'a> = ast::Expr<'s, Alloc<'a>>;
type ExprTy<'s, 'a> = ast::ExprTy<'s, Alloc<'a>>;

pub struct Parser<'src, 'a, 'ty> {
    src: &'src str,
    lexer: Lexer<'src>,
    id_interner: &'src mut Interner<'src, str>,
    ty_interner: &'ty mut TyInterner<'src, 'a>,
    ast_arena: Alloc<'a>,
    curr: Token,
    prev: Token,
    errors: Vec<SyntaxErrorWithCtx>,
    // Program data
    globals: Vec<GlobalVar<'src>>,
    types: TyStack<'src, 'a>,
}

impl<'src, 'a, 'ty> Parser<'src, 'a, 'ty> {
    pub fn new(
        src: &'src str,
        id_interner: &'src mut Interner<'src, str>,
        ty_interner: &'ty mut TyInterner<'src, 'a>,
        ast_arena: Alloc<'a>,
        types: TyStack<'src, 'a>,
    ) -> Self {
        Self {
            src,
            lexer: Lexer::new(src),
            id_interner,
            ty_interner,
            ast_arena,
            curr: Token::new(TokenTy::Eof, 0..0),
            prev: Token::new(TokenTy::Eof, 0..0),
            errors: Vec::new(),
            globals: Vec::new(),
            types,
        }
    }

    pub fn parse(mut self, src_path: PathBuf) -> Result<Program<'src, 'a>, CompileError> {
        let mut functions = Vec::new();

        self.advance_unchecked();
        while !self.at_end() {
            let Some(item) = self.item() else {
                return Err(CompileError::from_syntax_errors(
                    self.src,
                    src_path,
                    self.errors,
                ));
            };

            match item {
                Item::Fn { name, body } => {
                    functions.push(Function { name, body });
                }
            }
        }

        if !self.errors.is_empty() {
            Err(CompileError::from_syntax_errors(
                self.src,
                src_path,
                self.errors,
            ))
        } else {
            Ok(Program {
                globals: self.globals,
                functions,
            })
        }
    }
}

impl<'src, 'a, 'ty> Parser<'src, 'a, 'ty> {
    #[inline]
    fn alloc_stmt(&self, stmt: Stmt<'src, 'a>) -> Box<Stmt<'src, 'a>, Alloc<'a>> {
        Box::new_in(stmt, self.ast_arena)
    }

    #[inline]
    fn alloc_expr(&self, expr: Expr<'src, 'a>) -> Box<Expr<'src, 'a>, Alloc<'a>> {
        Box::new_in(expr, self.ast_arena)
    }

    #[inline]
    fn get_src_str(&self, token: &Token) -> &'src str {
        &self.src[token.ctx.clone()]
    }

    /// Returns an error from the previous token
    #[inline]
    fn error(&self, err: SyntaxError) -> SyntaxErrorWithCtx {
        SyntaxErrorWithCtx {
            ctx: self.prev.ctx.clone(),
            err,
        }
    }

    /// Returns an error from the current token
    #[inline]
    fn error_at(&self, err: SyntaxError) -> SyntaxErrorWithCtx {
        SyntaxErrorWithCtx {
            ctx: self.curr.ctx.clone(),
            err,
        }
    }

    #[inline]
    fn log_err(&mut self, err_with_ctx: SyntaxErrorWithCtx) {
        self.errors.push(err_with_ctx);
    }

    #[inline]
    fn at_end(&self) -> bool {
        matches!(self.curr.ty, TokenTy::Eof)
    }

    #[inline]
    fn check(&self, expected: TokenTy) -> bool {
        self.curr.ty == expected
    }

    #[inline]
    fn peek(&self) -> TokenTy {
        self.curr.ty
    }

    #[inline]
    fn intern_prev(&mut self) -> Interned<'src, str> {
        self.id_interner.intern(&self.src[self.prev.ctx.clone()])
    }

    #[inline]
    fn intern_next(&mut self) -> Interned<'src, str> {
        self.id_interner.intern(&self.src[self.curr.ctx.clone()])
    }

    fn synchronize(&mut self) {
        loop {
            // Check if start of decl
            if self.next_is_type() {
                break;
            }

            if matches!(self.prev.ty, TokenTy::Colon | TokenTy::CloseBrace) {
                break;
            }

            if matches!(
                self.curr.ty,
                TokenTy::OpenBrace | TokenTy::Semicolon | TokenTy::CloseBrace | TokenTy::If | TokenTy::Else | TokenTy::Return
            ) {
                break;
            }

            self.advance();
        }
    }

    fn advance_unchecked(&mut self) {
        let next = self.lexer.advance_token();
        let prev = std::mem::replace(&mut self.curr, next);
        self.prev = prev;
    }

    fn advance(&mut self) -> TokenTy {
        self.advance_unchecked();
        while let TokenTy::Err(err) = self.prev.ty {
            self.log_err(self.error(err));
            self.advance_unchecked();
        }
        self.prev.ty
    }

    /// Attempt to advance, consuming an expected tokenty
    /// - Logs an err and syncing on mismatch
    #[inline]
    fn eat(&mut self, expected: TokenTy, err: SyntaxError) -> bool {
        if self.check(expected) {
            self.advance_unchecked();
            true
        } else {
            self.log_err(self.error_at(err));
            self.synchronize();
            false
        }
    }

    /// Attempt to advance, consuming an expected tokenty
    /// - Logs an err on mismatch but doesnt sync
    #[inline]
    fn eat_no_sync(&mut self, expected: TokenTy, err: SyntaxError) -> bool {
        if self.check(expected) {
            self.advance_unchecked();
            true
        } else {
            self.log_err(self.error_at(err));
            false
        }
    }

    #[inline]
    fn eat_if(&mut self, expected: TokenTy) -> bool {
        if self.check(expected) {
            self.advance_unchecked();
            true
        } else {
            false
        }
    }

    /// Checks if the next token is a type or not
    ///
    /// Must check if an identifier is in the types stack because of typedef
    fn next_is_type(&mut self) -> bool {
        match self.peek() {
            TokenTy::Int => true,
            TokenTy::Identifier => {
                let id = self.intern_next();
                self.types.get(id).is_some()
            }
            _ => false,
        }
    }

    fn parse_type(&mut self) -> Interned<'a, Ty<'src, 'a>> {
        if !self.eat_if(TokenTy::Identifier) {
            self.eat(TokenTy::Int, SyntaxError::UnknownSymbol);
        }
        let id = self.intern_prev();

        self.types.get(id).copied().unwrap_or_else(|| {
            self.log_err(self.error(SyntaxError::UnknownSymbol));
            self.synchronize();
            self.ty_interner.intern(Ty::Poisoned)
        })
    }

    fn item(&mut self) -> Option<Item<'src, 'a>> {
        Some(self.function())
    }

    fn function(&mut self) -> Item<'src, 'a> {
        self.eat(TokenTy::Int, SyntaxError::UnknownSymbol);
        self.eat(TokenTy::Identifier, SyntaxError::ExpectedIdentifier);
        let name = self.intern_prev();

        self.eat(TokenTy::OpenParen, SyntaxError::ExpectedFunctionArgs);
        self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter);

        let Stmt::Block(body) = self.block() else {
            unreachable!("block parsing returned non-block stmt");
        };

        Item::Fn { name, body }
    }
}

/// Statements
impl<'src, 'a, 'ty> Parser<'src, 'a, 'ty> {
    fn declaration(&mut self) -> Option<Stmt<'src, 'a>> {
        if let TokenTy::Typedef = self.peek() {
            self.typedef();
            return None;
        }

        // Check if variable declaration
        if self.next_is_type() {
            Some(self.var_declaration())
        } else {
            Some(self.stmt())
        }
    }

    fn typedef(&mut self) {
        self.advance_unchecked();

        let ty = self.parse_type();
        self.eat(TokenTy::Identifier, SyntaxError::ExpectedIdentifier);
        let id = self.intern_prev();
        self.types.push(id, ty);
    }

    fn var_declaration(&mut self) -> Stmt<'src, 'a> {
        let ty = self.parse_type();
        self.eat(TokenTy::Identifier, SyntaxError::ExpectedIdentifier);
        let ctx = self.prev.ctx.clone();
        let name = self.intern_prev();
        let ident = Identifier { name, ctx };

        // Check for optional initializer
        let init = if self.eat_if(TokenTy::Equal) {
            let expr = self.expr();
            Some(self.alloc_expr(expr))
        } else {
            None
        };

        self.eat(TokenTy::Semicolon, SyntaxError::ExpectedSemicolon);
        Stmt::Decl(ident, ty, init)
    }

    fn stmt(&mut self) -> Stmt<'src, 'a> {
        match self.peek() {
            TokenTy::If => self.if_stmt(),
            TokenTy::OpenBrace => self.block(),
            TokenTy::Return => self.ret(),
            _ => self.expr_stmt(),
        }
    }

    fn if_stmt(&mut self) -> Stmt<'src, 'a> {
        self.advance_unchecked();

        self.eat_no_sync(TokenTy::OpenParen, SyntaxError::ExpectedOpenParen);
        let condition = self.expr();
        self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter);
        let then_branch = self.stmt();
        let else_branch = if self.eat_if(TokenTy::Else) {
            let else_branch = self.stmt();
            Some(self.alloc_stmt(else_branch))
        } else {
            None
        };

        Stmt::If(
            self.alloc_expr(condition),
            self.alloc_stmt(then_branch),
            else_branch,
        )
    }

    fn block(&mut self) -> Stmt<'src, 'a> {
        self.advance_unchecked();

        let old_scope_bottom = self.types.enter_scope();
        let mut stmts = Vec::new();

        while !self.at_end() && !self.check(TokenTy::CloseBrace) {
            if let Some(stmt) = self.declaration() {
                stmts.push(stmt);
            }
        }
        self.eat(TokenTy::CloseBrace, SyntaxError::UnclosedDelimiter);

        self.types.exit_scope(old_scope_bottom);
        Stmt::Block(stmts)
    }

    fn expr_stmt(&mut self) -> Stmt<'src, 'a> {
        if let Some(expr) = self.optional_expr(TokenTy::Semicolon, SyntaxError::ExpectedSemicolon) {
            Stmt::Expr(self.alloc_expr(expr))
        } else {
            Stmt::Nil
        }
    }

    fn ret(&mut self) -> Stmt<'src, 'a> {
        self.advance_unchecked();
        let expr = self.expr();
        self.eat(TokenTy::Semicolon, SyntaxError::ExpectedSemicolon);
        Stmt::Return(self.alloc_expr(expr))
    }

    fn poison_expr(&mut self, err: SyntaxError) -> Expr<'src, 'a> {
        if !matches!(self.peek(), TokenTy::Err(_)) {
            self.log_err(self.error_at(err));
        }
        let ctx = self.curr.ctx.clone();
        self.synchronize();
        Expr {
            expr: ExprTy::Poisoned,
            ctx,
        }
    }

    fn optional_expr(&mut self, ender: TokenTy, err: SyntaxError) -> Option<Expr<'src, 'a>> {
        if self.eat_if(ender) {
            None
        } else {
            let expr = self.expr();
            self.eat(TokenTy::Semicolon, err);
            Some(expr)
        }
    }

    #[inline]
    fn expr(&mut self) -> Expr<'src, 'a> {
        self.expr_with_precedence(Precedence::None.up())
    }

    fn expr_with_precedence(&mut self, prec: Precedence) -> Expr<'src, 'a> {
        let expr = self.prefix_expr();

        self.anyfix_expr(expr, prec)
    }

    #[inline]
    fn prefix_expr(&mut self) -> Expr<'src, 'a> {
        match self.peek() {
            TokenTy::PlusPlus
            | TokenTy::MinusMinus
            | TokenTy::Plus
            | TokenTy::Minus
            | TokenTy::Tilde
            | TokenTy::Bang => self.unary(),
            TokenTy::Const => self.constant(),
            TokenTy::Identifier => self.ident(),
            TokenTy::OpenParen => self.grouping(),
            _ => self.poison_expr(SyntaxError::InvalidExpr),
        }
    }

    #[inline]
    fn anyfix_expr(&mut self, mut expr: Expr<'src, 'a>, old_prec: Precedence) -> Expr<'src, 'a> {
        loop {
            let new_prec = self.peek().anyfix_precedence();
            if new_prec < old_prec {
                return expr;
            }
            self.advance_unchecked();

            expr = match new_prec {
                Precedence::Assignment => self.assignment(expr),
                Precedence::Conditional => self.ternary(expr),
                Precedence::Postfix => self.postfix(expr),
                new_prec => self.binary(expr, new_prec),
            }
        }
    }

    fn assignment(&mut self, lhs: Expr<'src, 'a>) -> Expr<'src, 'a> {
        let op = match self.prev.ty {
            TokenTy::Equal => AssignOp::Eq,
            TokenTy::PlusEqual => AssignOp::Add,
            TokenTy::MinusEqual => AssignOp::Sub,
            TokenTy::StarEqual => AssignOp::Mul,
            TokenTy::SlashEqual => AssignOp::Div,
            TokenTy::PercentEqual => AssignOp::Rem,
            TokenTy::LessLessEqual => AssignOp::Shl,
            TokenTy::GreaterGreaterEqual => AssignOp::Shr,
            TokenTy::AmpersandEqual => AssignOp::And,
            TokenTy::CaretEqual => AssignOp::Xor,
            TokenTy::PipeEqual => AssignOp::Or,
            _ => unreachable!("invalid tokenty {:?} reached assignment", self.prev.ty),
        };
        let rhs = self.expr_with_precedence(Precedence::Assignment);
        let ctx = Context::from_sub(lhs.ctx.clone(), rhs.ctx.clone());
        Expr {
            expr: ExprTy::Assign(op, self.alloc_expr(lhs), self.alloc_expr(rhs)),
            ctx,
        }
    }

    fn ternary(&mut self, cond: Expr<'src, 'a>) -> Expr<'src, 'a> {
        let then_branch = self.expr();
        self.eat(TokenTy::Colon, SyntaxError::ExpectedColon);
        let else_branch = self.expr_with_precedence(Precedence::Conditional);
        let ctx = Context::from_sub(cond.ctx.clone(), else_branch.ctx.clone());

        Expr {
            expr: ExprTy::Ternary(
                self.alloc_expr(cond),
                self.alloc_expr(then_branch),
                self.alloc_expr(else_branch),
            ),
            ctx,
        }
    }

    fn binary(&mut self, lhs: Expr<'src, 'a>, prec: Precedence) -> Expr<'src, 'a> {
        let op = match self.prev.ty {
            TokenTy::Star => BinaryOp::Mul,
            TokenTy::Slash => BinaryOp::Div,
            TokenTy::Percent => BinaryOp::Rem,
            // + -
            TokenTy::Plus => BinaryOp::Add,
            TokenTy::Minus => BinaryOp::Sub,
            // >> <<
            TokenTy::GreaterGreater => BinaryOp::Shr,
            TokenTy::LessLess => BinaryOp::Shl,
            // > >= < <=
            TokenTy::Greater => BinaryOp::G,
            TokenTy::GreaterEqual => BinaryOp::GE,
            TokenTy::Less => BinaryOp::L,
            TokenTy::LessEqual => BinaryOp::LE,
            // == !=
            TokenTy::EqualEqual => BinaryOp::E,
            TokenTy::BangEqual => BinaryOp::NE,
            // & ^ |
            TokenTy::Ampersand => BinaryOp::BitAnd,
            TokenTy::Caret => BinaryOp::BitXor,
            TokenTy::Pipe => BinaryOp::BitOr,
            // &&
            TokenTy::AmpersandAmpersand => BinaryOp::And,
            // ||
            TokenTy::PipePipe => BinaryOp::Or,
            _ => unreachable!("Unexpected TokenTy: {:?} made it to binary", self.prev.ty),
        };

        let rhs = self.expr_with_precedence(prec.up());
        let ctx = Context::from_sub(lhs.ctx.clone(), rhs.ctx.clone());
        Expr {
            expr: ExprTy::Binary(op, self.alloc_expr(lhs), self.alloc_expr(rhs)),
            ctx,
        }
    }

    fn unary(&mut self) -> Expr<'src, 'a> {
        self.advance_unchecked();

        let op = match self.prev.ty {
            TokenTy::Plus => UnaryOp::Plus,
            TokenTy::Minus => UnaryOp::Negate,
            TokenTy::Tilde => UnaryOp::Compliment,
            TokenTy::Bang => UnaryOp::Not,
            TokenTy::PlusPlus => UnaryOp::Increment,
            TokenTy::MinusMinus => UnaryOp::Decrement,
            _ => unreachable!(),
        };
        let op_ctx = self.prev.ctx.clone();
        let operand = self.expr_with_precedence(Precedence::Unary);
        let ctx = Context::from_sub(op_ctx, operand.ctx.clone());
        Expr {
            expr: ExprTy::Unary(op, self.alloc_expr(operand)),
            ctx,
        }
    }

    fn postfix(&mut self, operand: Expr<'src, 'a>) -> Expr<'src, 'a> {
        let ctx = Context::from_sub(operand.ctx.clone(), self.prev.ctx.clone());
        match self.prev.ty {
            TokenTy::PlusPlus => Expr {
                expr: ExprTy::DecInc(UnaryOp::Increment, self.alloc_expr(operand)),
                ctx,
            },
            TokenTy::Minus => Expr {
                expr: ExprTy::DecInc(UnaryOp::Decrement, self.alloc_expr(operand)),
                ctx,
            },
            _ => unreachable!("{:?}", self.prev.ty),
        }
    }

    fn constant(&mut self) -> Expr<'src, 'a> {
        self.advance_unchecked();
        match self.get_src_str(&self.prev).parse() {
            Ok(cnst) => Expr {
                expr: ExprTy::Constant(cnst),
                ctx: self.prev.ctx.clone(),
            },
            Err(_) => {
                self.log_err(self.error(SyntaxError::IntegerLiteralTooLarge));
                Expr {
                    expr: ExprTy::Poisoned,
                    ctx: self.prev.ctx.clone(),
                }
            }
        }
    }

    fn ident(&mut self) -> Expr<'src, 'a> {
        self.advance_unchecked();
        let id = self.intern_prev();
        Expr {
            expr: ExprTy::Var(id),
            ctx: self.prev.ctx.clone(),
        }
    }

    fn grouping(&mut self) -> Expr<'src, 'a> {
        self.advance_unchecked();
        let op_ctx = self.prev.ctx.clone();
        let Expr { expr, .. } = self.expr();
        self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter);
        let ctx = Context::from_sub(op_ctx, self.prev.ctx.clone());
        Expr { expr, ctx }
    }
}
