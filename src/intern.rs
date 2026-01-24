use std::{collections::HashSet, fmt::Debug, hash::Hash};

use crate::arena::TypedArena;

#[derive(Eq)]
pub struct Interned<'a, T: ?Sized + Eq + Hash>(&'a T);

impl<'a, T: ?Sized + Eq + Hash> Interned<'a, T> {
    pub fn get(&self) -> &'a T {
        self.0
    }
}

impl<'a, T: ?Sized + Eq + Hash> Debug for Interned<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Interned({:p})", self.0)
    }
}

impl<'a, T: ?Sized + Eq + Hash> PartialEq for Interned<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl<'a, T: ?Sized + Eq + Hash> Hash for Interned<'a, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.0 as *const T).addr().hash(state);
    }
}

impl<'a, T: ?Sized + Eq + Hash> Clone for Interned<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T: ?Sized + Eq + Hash> Copy for Interned<'a, T> {}

pub struct Interner<'a, T: ?Sized + Eq + Hash> {
    unique: HashSet<&'a T>,
}

impl<'a, T: ?Sized + Eq + Hash> Default for Interner<'a, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T: ?Sized + Eq + Hash> Interner<'a, T> {
    pub fn new() -> Self {
        Self {
            unique: HashSet::new(),
        }
    }

    pub fn intern(&mut self, item: &'a T) -> Interned<'a, T> {
        let &id = self.unique.get_or_insert(item);

        Interned(id)
    }

    pub fn len(&self) -> usize {
        self.unique.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<'a, T: Eq + Hash> Interner<'a, T> {
    /// Interns the type and saves memory by allocating in an arena
    ///
    /// Slightly less efficient compared to `intern` because of double key lookup
    pub fn intern_in_arena(&mut self, arena: &'a TypedArena<T>, item: T) -> Interned<'a, T> {
        let id = if !self.unique.contains(&item) {
            let allocated = arena.alloc(item);
            *self.unique.entry(allocated).insert().get()
        } else {
            *self.unique.get(&item).unwrap()
        };

        Interned(id)
    }
}

pub struct InternedArena<'a, T: Eq + Hash> {
    arena: &'a TypedArena<'a, T>,
    interner: Interner<'a, T>,
}

impl<'a, T: Eq + Hash> InternedArena<'a, T> {
    pub fn new(arena: &'a TypedArena<'a, T>) -> Self {
        Self {
            arena,
            interner: Interner::new(),
        }
    }

    pub fn intern(&mut self, item: T) -> Interned<'a, T> {
        self.interner.intern_in_arena(self.arena, item)
    }
}

#[test]
fn intern_test() {
    let mut interner = Interner::new();
    let id1 = interner.intern("Hello world!");
    let id2 = interner.intern("Wow");
    let id3 = interner.intern("3");
    let _ = interner.intern("4");
    let _ = interner.intern("5");
    let _ = interner.intern("6");
    let _ = interner.intern("7");
    let id3_again = interner.intern("3");
    let id1_again = interner.intern("Hello world!");
    let id2_again = interner.intern("Wow");
    let string = String::from("Hello");
    let _ = interner.intern(&string);

    assert_eq!(id1, id1_again);
    assert_eq!(id2, id2_again);
    assert_eq!(id3, id3_again);
    assert_eq!(interner.len(), 8);
}

#[test]
fn lookup_test() {
    let mut interner = Interner::new();
    let id1 = interner.intern("Hello world!");
    let id1_again = interner.intern("Hello world!");
    let id2 = interner.intern("Cool");

    assert_eq!(id1.get(), "Hello world!");
    assert_eq!(id1_again.get(), "Hello world!");
    assert_eq!(id2.get(), "Cool");

    assert_eq!(id1, id1_again);
}

#[test]
fn arena_intern_test() {
    use crate::arena::TypedArena;

    #[derive(PartialEq, Eq, Hash)]
    struct Person<'a> {
        name: Interned<'a, str>,
        age: usize,
        friend: Option<Interned<'a, Person<'a>>>,
    }
    let name = "Bob".to_string();

    let mut str_interner = Interner::<str>::new();
    let arena = TypedArena::new();
    let mut interner = Interner::new();

    let name_id1 = str_interner.intern(&name);
    let old_bob = Person {
        name: name_id1,
        age: 85,
        friend: None,
    };
    let person1 = interner.intern_in_arena(&arena, old_bob);

    let name_id2 = str_interner.intern(&name);
    let young_bob = Person {
        name: name_id2,
        age: 20,
        friend: Some(person1),
    };

    let name_id3 = str_interner.intern("Bob");
    let young_bob2 = Person {
        name: name_id3,
        age: 20,
        friend: Some(person1),
    };

    let person2 = interner.intern_in_arena(&arena, young_bob);
    let person3 = interner.intern_in_arena(&arena, young_bob2);

    assert_eq!(name_id1, name_id2);
    assert_ne!(person1, person2);
    assert_eq!(person2, person3);
}
