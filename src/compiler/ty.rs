use std::fmt::Display;

use crate::intern::{Interned, InternedArena, Interner};

pub type TyInterner<'s, 'a> = InternedArena<'a, 'static, Ty<'s, 'a>>;
pub type TyStack<'s, 'a> = ScopeStack<Interned<'s, str>, Interned<'a, Ty<'s, 'a>>>;

const BUILT_IN_TYPES: [(&str, Ty<'static, 'static>); 2] = [("int", Ty::Int), ("", Ty::Poisoned)];

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
    Poisoned,
}

impl Display for Ty<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Poisoned => write!(f, "poisoned"),
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
    scope_bottom: usize,
}

impl<K: PartialEq + Eq, V> ScopeStack<K, V> {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            scope_bottom: 0,
        }
    }

    /// Returns a value later used to reset the scope
    pub fn enter_scope(&mut self) -> usize {
        let old_scope_bottom = self.scope_bottom;
        self.scope_bottom = self.stack.len();
        old_scope_bottom
    }

    /// Takes in the value from enter scope and clears necessary (str, ty) pairs
    pub fn exit_scope(&mut self, old_scope_bottom: usize) {
        self.stack.drain(self.scope_bottom..);
        self.scope_bottom = old_scope_bottom;
    }

    pub fn push(&mut self, key: K, item: V) {
        self.stack.push((key, item));
    }

    pub fn in_scope(&self, key: &K) -> bool {
        self.stack[self.scope_bottom..]
            .iter()
            .any(|(k, _)| k == key)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.stack
            .iter()
            .rev()
            .find(|(k, _)| key == k)
            .map(|(_, val)| val)
    }
}
