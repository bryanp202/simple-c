use std::fmt::Display;

use crate::intern::{Interned, InternedArena, Interner};

pub type TyInterner<'s, 'a> = InternedArena<'a, 'static, Ty<'s, 'a>>;
pub type TyStack<'s, 'a> = ScopeStack<Interned<'s, str>, Interned<'a, Ty<'s, 'a>>>;

const BUILT_IN_TYPES: [(&'static str, Ty<'static, 'static>); 1] = [("int", Ty::Int)];

#[derive(PartialEq, Eq, Hash)]
pub enum Ty<'src, 'a> {
    Int,
    Function {
        ret: Interned<'a, Ty<'src, 'a>>,
        args: Vec<Interned<'a, Ty<'src, 'a>>>,
    },
    Adt {
        def: Interned<'src, str>,
        members: Vec<Interned<'a, Ty<'src, 'a>>>,
    },
}

impl<'src, 'a> Display for Ty<'src, 'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Function { .. } | Self::Adt { .. } => todo!("Ty display"),
        }
    }
}

pub fn built_in_types<'src, 'a>(
    id_interner: &mut Interner<'src, str>,
    ty_arena: &mut TyInterner<'src, 'a>,
) -> TyStack<'src, 'a> {
    let mut types = TyStack::new();
    for (name, ty) in BUILT_IN_TYPES {
        let interned_name = id_interner.intern(name);
        let interned_ty = ty_arena.intern(ty);
        types.push(interned_name, interned_ty);
    }
    types
}

pub struct ScopeStack<K: PartialEq + Eq, V> {
    stack: Vec<(K, V)>,
}

impl<K: PartialEq + Eq, V> ScopeStack<K, V> {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns a value later used to reset the scope
    pub fn enter_scope(&self) -> usize {
        self.stack.len()
    }

    /// Takes in the value from enter scope and clears necessary (str, ty) pairs
    pub fn exit_scope(&mut self, top: usize) {
        self.stack.drain(top..);
    }

    pub fn push(&mut self, key: K, item: V) {
        self.stack.push((key, item));
    }

    pub fn get(&self, key: K) -> Option<&V> {
        self.stack
            .iter()
            .rev()
            .find(|(k, _)| key == *k)
            .map(|(_, val)| val)
    }
}
