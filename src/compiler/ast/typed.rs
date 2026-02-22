use std::{
    alloc::{Allocator, Global},
    collections::HashMap,
};

use crate::{
    compiler::{
        asm::{Label, Linkage},
        ast::{AssignOp, BinaryOp, SpecifierFlags, UnaryOp},
        error::SemanticError,
        ty::Ty,
    },
    intern::Interned,
};

pub struct SymbolData<'src, 'a> {
    specifier_flags: SpecifierFlags,
    ty: Interned<'a, Ty<'src, 'a>>,
}

struct GlobalId<'src> {
    name: Interned<'src, str>,
    ctx: Option<Interned<'src, str>>,
}

pub struct Program<'src, 'a, A: Allocator = Global> {
    pub(crate) labels: u32,
    pub(crate) functions: Vec<Function<'src, 'a, A>>,
    pub(crate) globals: Vec<GlobalVar<'src>>,
}

pub struct GlobalVar<'src> {
    pub(crate) linkage: Linkage,
    pub(crate) name: Interned<'src, str>,
    pub(crate) generation: Option<u32>,
    pub(crate) def: Option<i32>,
}

pub struct Function<'src, 'a, A: Allocator = Global> {
    pub(crate) linkage: Linkage,
    pub(crate) name: Interned<'src, str>,
    pub(crate) body: Vec<Stmt<'src, 'a, A>>,
    pub(crate) param_count: u32,
    pub(crate) local_count: u32,
}

pub struct ForStmt<'src, 'a, A: Allocator> {
    pub(crate) init: Option<Box<Stmt<'src, 'a, A>, A>>,
    pub(crate) condition: Option<Box<Expr<'src, 'a, A>, A>>,
    pub(crate) increment: Option<Box<Expr<'src, 'a, A>, A>>,
    pub(crate) body: Box<Stmt<'src, 'a, A>, A>,
}

pub struct SwitchStmt<'src, 'a, A: Allocator> {
    pub(crate) expr: Box<Expr<'src, 'a, A>, A>,
    pub(crate) cases: Vec<SwitchCase>,
    pub(crate) default: Option<Label>,
    pub(crate) body: Box<Stmt<'src, 'a, A>, A>,
}

pub struct SwitchCase {
    pub(crate) val: i32,
    pub(crate) label: Label,
}

pub enum Stmt<'src, 'a, A: Allocator = Global> {
    Break,
    Block(Vec<Stmt<'src, 'a, A>>),
    Continue,
    Decl(Option<Box<Expr<'src, 'a, A>, A>>),
    Do(Box<Stmt<'src, 'a, A>, A>, Box<Expr<'src, 'a, A>, A>),
    Expr(Box<Expr<'src, 'a, A>, A>),
    For(Box<ForStmt<'src, 'a, A>, A>),
    Goto(Label),
    If(
        Box<Expr<'src, 'a, A>, A>,
        Box<Stmt<'src, 'a, A>, A>,
        Option<Box<Stmt<'src, 'a, A>, A>>,
    ),
    Labled(Label, Box<Stmt<'src, 'a, A>, A>),
    Nil,
    Return(Box<Expr<'src, 'a, A>, A>),
    Switch(Box<SwitchStmt<'src, 'a, A>, A>),
    While(Box<Expr<'src, 'a, A>, A>, Box<Stmt<'src, 'a, A>, A>),
}

pub struct Expr<'src, 'a, A: Allocator> {
    pub(crate) expr: ExprTy<'src, 'a, A>,
    pub(crate) ty: Interned<'a, Ty<'src, 'a>>,
}

pub struct CallExpr<'src, 'a, A: Allocator> {
    pub(crate) operand: Box<Expr<'src, 'a, A>, A>,
    pub(crate) args: Vec<Box<Expr<'src, 'a, A>, A>, A>,
}

pub enum ExprTy<'src, 'a, A: Allocator> {
    Ternary(
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
    ),
    Assign(
        AssignOp,
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
    ), // Op, lhs, rhs
    Binary(
        BinaryOp,
        Box<Expr<'src, 'a, A>, A>,
        Box<Expr<'src, 'a, A>, A>,
    ), // Op, lhs, rhs
    Unary(UnaryOp, Box<Expr<'src, 'a, A>, A>), // Op, operand
    DecInc(UnaryOp, Box<Expr<'src, 'a, A>, A>), // Op, operand (op is either ++ or --)
    Call(Box<CallExpr<'src, 'a, A>, A>),
    FnLoad(Interned<'src, str>),
    GlobalLoad(Interned<'src, str>),
    LocalStaticLoad(u32),
    Local(u32),
    Constant(i32),
    Poisoned,
}

pub struct SymbolTable<'src, 'a, A: Allocator> {
    symbols: HashMap<Interned<'src, str>, SymbolData<'src, 'a>>,
    functions: Vec<Function<'src, 'a, A>>,
    globals: Vec<GlobalVar<'src>>,
    //globals: Vec<GlobalId<'src>>,
}

impl<'src, 'a, A: Allocator> SymbolTable<'src, 'a, A> {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            functions: Vec::new(),
            globals: Vec::new(),
        }
    }

    pub fn into_parts(mut self) -> (Vec<Function<'src, 'a, A>>, Vec<GlobalVar<'src>>) {
        for (name, data) in self.symbols {
            if !data.specifier_flags.intersects(SpecifierFlags::Extern | SpecifierFlags::Defined) {
                self.globals.push(GlobalVar { linkage: Linkage::None, name, generation: None, def: None });
            }
        }

        (self.functions, self.globals)
    }

    pub fn decl(
        &mut self,
        name: Interned<'src, str>,
        specifier_flags: SpecifierFlags,
        ty: Interned<'a, Ty<'src, 'a>>,
    ) -> Result<Linkage, SemanticError> {
        let symbol = self.symbols.entry(name).or_insert(SymbolData {
            specifier_flags,
            ty,
        });

        if symbol.ty != ty {
            return Err(SemanticError::TypeMismatch);
        }

        if specifier_flags.contains(SpecifierFlags::Extern | SpecifierFlags::Static) {
            return Err(SemanticError::TooManyStorageClasses);
        }

        if symbol.specifier_flags.contains(SpecifierFlags::Static) {
            if specifier_flags.contains(SpecifierFlags::Extern) {
                Err(SemanticError::DeclWithDifLinkage)
            } else {
                Ok(Linkage::None)
            }
        } else if specifier_flags.contains(SpecifierFlags::Static) {
            Err(SemanticError::DeclWithDifLinkage)
        } else {
            Ok(Linkage::Global)
        }
    }

    pub fn decl_local_static(
        &mut self,
        name: Interned<'src, str>,
        generation: u32,
        def: Option<i32>,
    ) {
        self.globals.push(GlobalVar {
            linkage: Linkage::None,
            name,
            generation: Some(generation),
            def,
        });
    }

    /// Must be called after `decl`
    /// Panics otherwise
    pub fn define_fun(&mut self, fun: Function<'src, 'a, A>) -> Result<(), SemanticError> {
        let symbol = self
            .symbols
            .get_mut(&fun.name)
            .expect("expected define to be called after decl");
        if symbol.specifier_flags.contains(SpecifierFlags::Defined) {
            Err(SemanticError::DuplicateDef)
        } else {
            symbol.specifier_flags.insert(SpecifierFlags::Defined);
            self.functions.push(fun);
            Ok(())
        }
    }

    /// Must be called after `decl`
    /// Panics otherwise
    pub fn define_global(&mut self, global: GlobalVar<'src>) -> Result<(), SemanticError> {
        let symbol = self
            .symbols
            .get_mut(&global.name)
            .expect("expected define to be called after decl");
        if symbol.specifier_flags.contains(SpecifierFlags::Defined) {
            Err(SemanticError::DuplicateDef)
        } else {
            symbol.specifier_flags.insert(SpecifierFlags::Defined);
            self.globals.push(global);
            Ok(())
        }
    }
}
