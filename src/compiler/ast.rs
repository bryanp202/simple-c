use crate::{compiler::token::Token, intern::InternedStr};

pub struct Program {
    pub(crate) item: Item,
}

pub enum Item {
    Fn { name: Token, body: Stmt },
    //Fn { name: InternedStr, body: Stmt },
}

pub enum Stmt {
    Return(Expr),
}

pub enum Expr {
    Constant(i32),
}
