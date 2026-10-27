use super::Component;
use crate::hash::StateHasher;
use core::any::Any;

const ABSENT: u32 = u32::MAX;

pub trait ErasedStore: Any {
    fn name(&self) -> &'static str;
    fn len(&self) -> usize;
    fn remove_entity(&mut self, index: u32);
    fn hash_into(&self, hasher: &mut StateHasher);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Clone, Debug)]
pub struct SparseSet<T> {
    sparse: Vec<u32>,
    entities: Vec<u32>,
    data: Vec<T>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            sparse: Vec::new(),
            entities: Vec::new(),
            data: Vec::new(),
        }
    }
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn position(&self, index: u32) -> Option<usize> {
        match self.sparse.get(index as usize) {
            Some(&position) if position != ABSENT => Some(position as usize),
            _ => None,
        }
    }

    pub fn contains(&self, index: u32) -> bool {
        self.position(index).is_some()
    }

    pub fn get(&self, index: u32) -> Option<&T> {
        self.position(index).map(|position| &self.data[position])
    }

    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        self.position(index)
            .map(|position| &mut self.data[position])
    }

    pub fn insert(&mut self, index: u32, value: T) -> Option<T> {
        if let Some(position) = self.position(index) {
            return Some(core::mem::replace(&mut self.data[position], value));
        }
        if self.sparse.len() <= index as usize {
            self.sparse.resize(index as usize + 1, ABSENT);
        }
        self.sparse[index as usize] = self.data.len() as u32;
        self.entities.push(index);
        self.data.push(value);
        None
    }

    pub fn remove(&mut self, index: u32) -> Option<T> {
        let position = self.position(index)?;
        let last = self.data.len() - 1;
        self.entities.swap(position, last);
        let value = self.data.swap_remove(position);
        self.entities.pop();
        if position != last {
            let moved = self.entities[position];
            self.sparse[moved as usize] = position as u32;
        }
        self.sparse[index as usize] = ABSENT;
        Some(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        self.entities.iter().copied().zip(self.data.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut T)> + '_ {
        self.entities.iter().copied().zip(self.data.iter_mut())
    }
}

impl<T: Component> ErasedStore for SparseSet<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn remove_entity(&mut self, index: u32) {
        self.remove(index);
    }

    fn hash_into(&self, hasher: &mut StateHasher) {
        hasher.write_len_prefixed(T::NAME.as_bytes());
        hasher.write_u64(self.data.len() as u64);
        for (index, value) in self.iter() {
            hasher.write_u32(index);
            value.hash_into(hasher);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_replace() {
        let mut set = SparseSet::new();
        assert_eq!(set.insert(5, 10), None);
        assert_eq!(set.insert(5, 11), Some(10));
        assert_eq!(set.get(5), Some(&11));
        assert_eq!(set.get(4), None);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn iteration_follows_insertion_order() {
        let mut set = SparseSet::new();
        set.insert(9, 'a');
        set.insert(2, 'b');
        set.insert(7, 'c');
        let order: Vec<(u32, char)> = set.iter().map(|(i, v)| (i, *v)).collect();
        assert_eq!(order, vec![(9, 'a'), (2, 'b'), (7, 'c')]);
    }

    #[test]
    fn remove_swaps_last_into_the_hole() {
        let mut set = SparseSet::new();
        set.insert(1, 'a');
        set.insert(2, 'b');
        set.insert(3, 'c');
        assert_eq!(set.remove(1), Some('a'));
        let order: Vec<(u32, char)> = set.iter().map(|(i, v)| (i, *v)).collect();
        assert_eq!(order, vec![(3, 'c'), (2, 'b')]);
        assert_eq!(set.get(3), Some(&'c'));
        assert_eq!(set.get(2), Some(&'b'));
        assert_eq!(set.remove(1), None);
    }

    #[test]
    fn remove_last_and_only_elements() {
        let mut set = SparseSet::new();
        set.insert(0, 1);
        set.insert(1, 2);
        assert_eq!(set.remove(1), Some(2));
        assert_eq!(set.remove(0), Some(1));
        assert!(set.is_empty());
        assert!(!set.contains(0));
    }

    #[test]
    fn get_mut_and_iter_mut_edit_in_place() {
        let mut set = SparseSet::new();
        set.insert(0, 1);
        set.insert(1, 2);
        *set.get_mut(0).unwrap() += 10;
        for (_, value) in set.iter_mut() {
            *value *= 2;
        }
        assert_eq!(set.get(0), Some(&22));
        assert_eq!(set.get(1), Some(&4));
    }
}
