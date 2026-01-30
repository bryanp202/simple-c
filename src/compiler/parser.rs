use std::path::PathBuf;

use crate::{
    arena::Arena,
    compiler::{
        ast::{self, AssignOp, BinaryOp, Function, GlobalVar, UnaryOp},
        error::{CompileError, SyntaxError, SyntaxErrorWithCtx},
        lexer::Lexer,
        token::{Precedence, Token, TokenTy},
    },
    intern::Interner,
};

type Alloc<'a> = &'a Arena<'static>;
type Program<'s, 'a> = ast::Program<'s, Alloc<'a>>;
type Item<'s, 'a> = ast::Item<'s, Alloc<'a>>;
type Stmt<'s, 'a> = ast::Stmt<'s, Alloc<'a>>;
type Expr<'s, 'a> = ast::Expr<'s, Alloc<'a>>;

pub struct Parser<'src, 'a> {
    src: &'src str,
    lexer: Lexer<'src>,
    id_interner: &'src mut Interner<'src, str>,
    ast_arena: Alloc<'a>,
    curr: Token,
    prev: Token,
    errors: Vec<SyntaxErrorWithCtx>,
    // Program data
    globals: Vec<GlobalVar<'src>>,
}

impl<'src, 'a> Parser<'src, 'a> {
    pub fn new(
        src: &'src str,
        id_interner: &'src mut Interner<'src, str>,
        ast_arena: &'a Arena<'static>,
    ) -> Self {
        Self {
            src,
            lexer: Lexer::new(src),
            id_interner,
            ast_arena,
            curr: Token::new(TokenTy::Eof, 0..0),
            prev: Token::new(TokenTy::Eof, 0..0),
            errors: Vec::new(),
            globals: Vec::new(),
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
                    functions.push(Function { name, body, local_count: 0 });
                }
            }
        }

        let program = Program {
            globals: self.globals,
            functions,
        };
        Ok(program)
    }
}

impl<'src, 'a> Parser<'src, 'a> {
    #[inline]
    fn alloc_expr(&self, expr: Expr<'src, 'a>) -> Box<Expr<'src, 'a>, Alloc<'a>> {
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

    fn item(&mut self) -> Option<Item<'src, 'a>> {
        match self.function() {
            Ok(fun) => Some(fun),
            Err(err) => {
                self.log_err(err);
                None
            }
        }
    }

    fn function(&mut self) -> Result<Item<'src, 'a>, SyntaxErrorWithCtx> {
        self.eat(TokenTy::Int, SyntaxError::UnknownSymbol)?;
        self.eat(TokenTy::Identifier, SyntaxError::ExpectedIdentifier)?;
        let name_token = self.prev.clone();
        let name = self.id_interner.intern(&self.src[name_token.ctx]);

        self.eat(TokenTy::OpenParen, SyntaxError::ExpectedFunctionArgs)?;
        self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter)?;
        self.eat(TokenTy::OpenBrace, SyntaxError::UnterminatedBlockComment)?;

        let mut body = Vec::new();
        while !self.at_end() && !self.check(TokenTy::CloseBrace) {
            body.push(self.stmt()?);
        }

        self.eat(TokenTy::CloseBrace, SyntaxError::UnclosedDelimiter)?;

        Ok(Item::Fn { name, body })
    }

    fn stmt(&mut self) -> Result<Stmt<'src, 'a>, SyntaxErrorWithCtx> {
        match self.peek() {
            TokenTy::Semicolon => {
                self.advance_unchecked();
                Ok(Stmt::Nil)
            },
            TokenTy::Return => self.ret_stmt(),
            _ => self.expr_stmt(),
        }       
    }

    fn expr_stmt(&mut self) -> Result<Stmt<'src, 'a>, SyntaxErrorWithCtx> {
        let expr = self.expr()?;
        self.eat(TokenTy::Semicolon, SyntaxError::ExpectedSemicolon)?;
        Ok(Stmt::Expr(self.alloc_expr(expr)))
    }

    fn ret_stmt(&mut self) -> Result<Stmt<'src, 'a>, SyntaxErrorWithCtx> {
        self.advance_unchecked();
        let expr = self.expr()?;
        self.eat(TokenTy::Semicolon, SyntaxError::ExpectedSemicolon)?;
        Ok(Stmt::Return(self.alloc_expr(expr)))
    }

    #[inline]
    fn expr(&mut self) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        self.expr_with_precedence(Precedence::None.up())
    }

    fn expr_with_precedence(
        &mut self,
        prec: Precedence,
    ) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        let expr = self.prefix_expr()?;

        self.anyfix_expr(expr, prec)
    }

    #[inline]
    fn prefix_expr(&mut self) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        match self.advance()? {
            TokenTy::Minus | TokenTy::Tilde | TokenTy::Bang => self.unary(),
            TokenTy::Const => self.constant(),
            TokenTy::OpenParen => self.grouping(),
            _ => Err(self.error(SyntaxError::InvalidExpr)),
        }
    }

    #[inline]
    fn anyfix_expr(
        &mut self,
        mut expr: Expr<'src, 'a>,
        old_prec: Precedence,
    ) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        loop {
            let new_prec = self.peek().anyfix_precedence();
            if new_prec < old_prec {
                return Ok(expr);
            }
            self.advance_unchecked();

            expr = match new_prec {
                Precedence::Assignment => self.assignment(expr)?,
                Precedence::Postfix => todo!("postfix"),
                new_prec => self.binary(expr, new_prec)?,
            }
        }
    }

    fn assignment(&mut self, lhs: Expr<'src, 'a>) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
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
        let rhs = self.expr_with_precedence(Precedence::Assignment)?;
        Ok(Expr::Assign(op, self.alloc_expr(lhs), self.alloc_expr(rhs)))
    }

    fn binary(
        &mut self,
        lhs: Expr<'src, 'a>,
        prec: Precedence,
    ) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
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

        let rhs = self.expr_with_precedence(prec.up())?;
        Ok(Expr::Binary(op, self.alloc_expr(lhs), self.alloc_expr(rhs)))
    }

    fn unary(&mut self) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        let op = match self.prev.ty {
            TokenTy::Minus => UnaryOp::Negate,
            TokenTy::Tilde => UnaryOp::Compliment,
            TokenTy::Bang => UnaryOp::Not,
            _ => unreachable!(),
        };
        let operand = self.expr_with_precedence(Precedence::Unary)?;
        Ok(Expr::Unary(op, self.alloc_expr(operand)))
    }

    fn constant(&mut self) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        let cnst = self
            .get_src_str(&self.prev)
            .parse()
            .map_err(|_| self.error(SyntaxError::IntegerLiteralTooLarge))?;

        Ok(Expr::Constant(cnst))
    }

    fn grouping(&mut self) -> Result<Expr<'src, 'a>, SyntaxErrorWithCtx> {
        let expr = self.expr()?;
        self.eat(TokenTy::CloseParen, SyntaxError::UnclosedDelimiter)?;
        Ok(expr)
    }
}
