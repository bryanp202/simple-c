use std::path::PathBuf;

use crate::compiler::{
    ast::{Expr, Item, Program, Stmt},
    error::{CompileError, SyntaxError, SyntaxErrorWithCtx},
    lexer::Lexer,
    token::{Token, TokenTy},
};

pub struct Parser<'src> {
    src: &'src str,
    lexer: Lexer<'src>,
    curr: Token,
    prev: Token,
    errors: Vec<SyntaxErrorWithCtx>,
}

impl<'src> Parser<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src,
            lexer: Lexer::new(src),
            curr: Token {
                ty: TokenTy::Err(SyntaxError::UnknownSymbol),
                range: 0..0,
            },
            prev: Token {
                ty: TokenTy::Err(SyntaxError::UnknownSymbol),
                range: 0..0,
            },
            errors: Vec::new(),
        }
    }

    pub fn parse(mut self, src_path: PathBuf) -> Result<Program, CompileError> {
        self.advance();
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

impl<'src> Parser<'src> {
    fn get_src_str(&self, token: &Token) -> &str {
        &self.src[token.range.clone()]
    }

    fn log_err(&mut self, err_with_ctx: SyntaxErrorWithCtx) {
        self.errors.push(err_with_ctx);
    }

    fn at_end(&self) -> bool {
        matches!(self.curr.ty, TokenTy::Eof)
    }

    fn advance(&mut self) {
        let next = self.lexer.advance_token();
        let prev = std::mem::replace(&mut self.curr, next);
        self.prev = prev;
    }

    fn check(&mut self, expected: TokenTy) -> bool {
        self.curr.ty == expected
    }

    fn eat(&mut self, expected: TokenTy, err: SyntaxError) -> Result<(), SyntaxErrorWithCtx> {
        if self.check(expected) {
            self.advance();
            Ok(())
        } else {
            Err(SyntaxErrorWithCtx {
                ctx: self.curr.range.clone(),
                err,
            })
        }
    }

    fn item(&mut self) -> Option<Item> {
        match self.function() {
            Ok(fun) => Some(fun),
            Err(err) => {
                self.log_err(err);
                None
            }
        }
    }

    fn function(&mut self) -> Result<Item, SyntaxErrorWithCtx> {
        self.eat(TokenTy::Int, SyntaxError::UnknownSymbol)?;
        self.eat(TokenTy::Identifier, SyntaxError::InvalidIntegerSuffix)?;
        let name_token = self.prev.clone();
        let name = self.src[name_token.range].to_string();

        self.eat(TokenTy::OpenParen, SyntaxError::AdjacentDigitSeperators)?;
        self.eat(TokenTy::CloseParen, SyntaxError::AdjacentDigitSeperators)?;
        self.eat(TokenTy::OpenBrace, SyntaxError::UnterminatedBlockComment)?;

        let body = self.stmt()?;

        self.eat(TokenTy::CloseBrace, SyntaxError::UnknownSymbol)?;
        

        Ok(Item::Fn { name, body })
    }

    fn stmt(&mut self) -> Result<Stmt, SyntaxErrorWithCtx> {
        self.eat(TokenTy::Return, SyntaxError::AdjacentDigitSeperators)?;
        let expr = self.expr()?;
        self.eat(TokenTy::Semicolon, SyntaxError::AdjacentDigitSeperators)?;

        Ok(Stmt::Return(expr))
    }

    fn expr(&mut self) -> Result<Expr, SyntaxErrorWithCtx> {
        self.eat(TokenTy::Const, SyntaxError::AdjacentDigitSeperators)?;
        let cnst = self.get_src_str(&self.prev).parse().unwrap();

        Ok(Expr::Constant(cnst))
    }
}
