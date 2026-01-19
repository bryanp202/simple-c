use std::collections::HashMap;

type InternedInternal = u32;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct InternedStr(InternedInternal);

pub struct StrInterner<'a> {
    ids: HashMap<&'a str, usize>,
    unique: Vec<&'a str>,
}

impl<'a> Default for StrInterner<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> StrInterner<'a> {
    pub fn new() -> Self {
        Self {
            ids: HashMap::new(),
            unique: Vec::new(),
        }
    }

    pub fn intern(&mut self, key: &'a str) -> InternedStr {
        let &mut id = self.ids.entry(key).or_insert_with(|| {
            let id = self.unique.len();
            self.unique.push(key);
            id
        });

        InternedStr(id as InternedInternal)
    }

    pub fn lookup(&self, id: InternedStr) -> &'a str {
        self.unique[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.unique.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[test]
fn intern_test() {
    let mut interner = StrInterner::new();
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
    let mut interner = StrInterner::new();
    let id1 = interner.intern("Hello world!");
    let id1_again = interner.intern("Hello world!");
    let id2 = interner.intern("Cool");

    assert_eq!(interner.lookup(id1), "Hello world!");
    assert_eq!(interner.lookup(id1_again), "Hello world!");
    assert_eq!(interner.lookup(id2), "Cool");

    assert_eq!(id1, id1_again);
}
