use std::fmt;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

/// Trait implemented by entity index types (wrappers around u32).
pub trait EntityId: Copy + Clone + PartialEq + Eq + std::hash::Hash + fmt::Debug {
    fn from_u32(idx: u32) -> Self;
    fn as_u32(self) -> u32;
    fn index(self) -> usize {
        self.as_u32() as usize
    }
}

/// Helper macro to define strongly-typed index structures.
#[macro_export]
macro_rules! define_entity {
    ($name:ident, $prefix:expr) => {
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl $crate::arena::EntityId for $name {
            #[inline]
            fn from_u32(idx: u32) -> Self {
                Self(idx)
            }
            #[inline]
            fn as_u32(self) -> u32 {
                self.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }
    };
}

define_entity!(Value, "%");
define_entity!(Inst, "inst");
define_entity!(Block, "block");
define_entity!(FuncId, "func");

/// An arena map that assigns sequential, strongly-typed EntityId keys to values of type V.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Arena<K: EntityId, V> {
    data: Vec<V>,
    _marker: PhantomData<K>,
}

impl<K: EntityId, V> Default for Arena<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: EntityId, V> Arena<K, V> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    /// Push a new value into the arena and return its assigned ID.
    pub fn push(&mut self, value: V) -> K {
        let id = K::from_u32(self.data.len() as u32);
        self.data.push(value);
        id
    }

    pub fn get(&self, id: K) -> Option<&V> {
        self.data.get(id.index())
    }

    pub fn get_mut(&mut self, id: K) -> Option<&mut V> {
        self.data.get_mut(id.index())
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Iterate over (ID, &value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.data
            .iter()
            .enumerate()
            .map(|(i, v)| (K::from_u32(i as u32), v))
    }

    /// Iterate over (ID, &mut value) pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.data
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (K::from_u32(i as u32), v))
    }

    /// Iterate over references to values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.data.iter()
    }

    /// Iterate over mutable references to values.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.data.iter_mut()
    }

    /// Return a slice of the underlying vector.
    pub fn as_slice(&self) -> &[V] {
        &self.data
    }
}

impl<K: EntityId, V: fmt::Debug> fmt::Debug for Arena<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: EntityId, V> Index<K> for Arena<K, V> {
    type Output = V;

    fn index(&self, id: K) -> &Self::Output {
        &self.data[id.index()]
    }
}

impl<K: EntityId, V> IndexMut<K> for Arena<K, V> {
    fn index_mut(&mut self, id: K) -> &mut Self::Output {
        &mut self.data[id.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena() {
        let mut arena: Arena<Value, String> = Arena::new();
        let v0 = arena.push("hello".to_string());
        let v1 = arena.push("world".to_string());
        assert_eq!(v0.as_u32(), 0);
        assert_eq!(v1.as_u32(), 1);
        assert_eq!(arena[v0], "hello");
        assert_eq!(arena[v1], "world");
        assert_eq!(arena.len(), 2);
    }
}
