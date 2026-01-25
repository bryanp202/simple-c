use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, UnaryOp},
        error::{CompileError, SyntaxError, SyntaxErrorWithCtx},
        lexer::Lexer,
        token::{Token, TokenTy},
    },
    intern::Interner,
};

type Alloc<'a> = &'a Arena<'static>;
type Program<'s, 'a> = ast::Program<'s, Alloc<'a>>;
type Item<'s, 'a> = ast::Item<'s, Alloc<'a>>;
type Stmt<'a> = ast::Stmt<Alloc<'a>>;
type Expr<'a> = ast::Expr<Alloc<'a>>;

pub struct Parser<'src, 'arena> {
    src: &'src str,
    lexer: Lexer<'src>,
    id_interner: &'src mut Interner<'src, str>,
    ast_arena: Alloc<'arena>,
    curr: Token,
    prev: Token,
    errors: Vec<SyntaxErrorWithCtx>,
}

impl<'src, 'arena> Parser<'src, 'arena> {
    pub fn new(
        src: &'src str,
        id_interner: &'src mut Interner<'src, str>,
        ast_arena: &'arena Arena<'static>,
    ) -> Self {
        Self {
            src,
            lexer: Lexer::new(src),
            id_interner,
            ast_arena,
            curr: Token::new(TokenTy::Eof, 0..0),
            prev: Token::new(TokenTy::Eof, 0..0),
            errors: Vec::new(),
        }
    }

    pub fn parse(mut self, src_path: PathBuf) -> Result<Program<'src, 'arena>, CompileError> {
        self.advance_unchecked();
        let Some(item) = self.item() else {
            return Err(CompileError::from_syntax_errors(
                self.src,
                src_path,
                self.errors,
            ));
        };
        let program = Program { item };
        Ok(program)
    }
}

impl<'src, 'arena> Parser<'src, 'arena> {
    #[inline]
    fn alloc_expr(&self, expr: Expr<'arena>) -> Box<Expr<'arena>, Alloc<'arena>> {
        Box::new_in(expr, self.ast_arena)
    }

    #[inline]
    fn get_src_str(&self, token: &Token) -> &str {
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

    fn check_any(&self, iter: impl IntoIterator<Item = TokenTy>) -> bool {
        iter.into_iter().any(|expected| self.check(expected))
    }

    #[inline]
    fn peek(&self) -> TokenTy {
        self.curr.ty
    }

    fn advance_unchecked(&mut self) {
        let next = self.lexer.advance_token();
        let prev = std::mem::replace(&mut self.curr, next);
        self.prev = prev;
    }

    fn advance(&mut self) -> Result<TokenTy, SyntaxErrorWithCtx> {
        if let TokenTy::Err(err) = self.curr.ty {
            Err(self.error_at(err))
        } else {
            self.advance_unchecked();
            Ok(self.prev.ty)
        }
    }

    fn eat(&mut self, expected: TokenTy, err: SyntaxError) -> Result<(), SyntaxErrorWithCtx> {
        if self.check(expected) {
            self.advance_unchecked();
            Ok(())
        } else {
            self.advance()?;
            Err(self.error(err))
        }
    }

    fn item(&mut self) -> Option<Item<'src, 'arena>> {
        match self.function() {
            Ok(fun) => Some(fun),
            Err(err) => {
                self.log_err(err);
                None
            }
        }
    }

    fn function(&mut self) -> Result<Item<'src, 'arena>, SyntaxErrorWithCtx> {
        self.eat(TokenTy::Int, SyntaxError::UnknownSymbol)?;
        self.eat(TokenTy::Identifier, SyntaxError::InvalidIntegerSuffix)?;
        let name_token = self.prev.clone();
        let name = self.id_interner.intern(&self.src[name_token.ctx]);

        self.eat(TokenTy::OpenParen, SyntaxError::ExpectedFunctionArgs)?;
        self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter)?;
        self.eat(TokenTy::OpenBrace, SyntaxError::UnterminatedBlockComment)?;

        let body = self.stmt()?;

        self.eat(TokenTy::CloseBrace, SyntaxError::UnknownSymbol)?;

        Ok(Item::Fn { name, body })
    }

    fn stmt(&mut self) -> Result<Stmt<'arena>, SyntaxErrorWithCtx> {
        self.eat(TokenTy::Return, SyntaxError::InvalidExpr)?;
        let expr = self.expr()?;
        self.eat(TokenTy::Semicolon, SyntaxError::ExpectedSemicolon)?;

        Ok(Stmt::Return(self.alloc_expr(expr)))
    }

    #[inline]
    fn expr(&mut self) -> Result<Expr<'arena>, SyntaxErrorWithCtx> {
        self.expr_with_precedence(0)
    }

    fn expr_with_precedence(&mut self, prec: usize) -> Result<Expr<'arena>, SyntaxErrorWithCtx> {
        let mut lhs = self.unary()?;

        while let Some((new_prec, op)) = self.peek().binary_prec()
            && new_prec > prec
        {
            self.advance_unchecked();
            let rhs = self.expr_with_precedence(new_prec)?;
            let new_lhs = Expr::Binary(op, self.alloc_expr(lhs), self.alloc_expr(rhs));
            lhs = new_lhs;
        }

        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr<'arena>, SyntaxErrorWithCtx> {
        const UNARY_OPS: [TokenTy; 2] = [TokenTy::Minus, TokenTy::Tilde];

        if self.check_any(UNARY_OPS) {
            self.advance_unchecked();
            let op = match self.prev.ty {
                TokenTy::Minus => UnaryOp::Negate,
                TokenTy::Tilde => UnaryOp::Compliment,
                _ => unreachable!(),
            };
            let operand = self.unary()?;
            Ok(Expr::Unary(op, self.alloc_expr(operand)))
        } else {
            self.literal()
        }
    }

    fn literal(&mut self) -> Result<Expr<'arena>, SyntaxErrorWithCtx> {
        let ty = self.advance()?;

        match ty {
            TokenTy::Const => self.constant(),
            TokenTy::OpenParen => {
                let expr = self.expr()?;
                self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter)?;
                Ok(expr)
            }
            _ => Err(self.error(SyntaxError::InvalidExpr)),
        }
    }

    fn constant(&mut self) -> Result<Expr<'arena>, SyntaxErrorWithCtx> {
        let cnst = self
            .get_src_str(&self.prev)
            .parse()
            .map_err(|_| self.error(SyntaxError::IntegerLiteralTooLarge))?;

        Ok(Expr::Constant(cnst))
    }
}
