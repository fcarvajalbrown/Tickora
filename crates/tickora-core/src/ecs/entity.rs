use crate::hash::StateHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    pub const fn new(index: u32, generation: u32) -> Entity {
        Entity { index, generation }
    }

    pub const fn index(self) -> u32 {
        self.index
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Debug, Default)]
pub struct EntityAllocator {
    generations: Vec<u32>,
    alive: Vec<bool>,
    free: Vec<u32>,
    alive_count: usize,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&mut self) -> Entity {
        self.alive_count += 1;
        match self.free.pop() {
            Some(index) => {
                self.alive[index as usize] = true;
                Entity::new(index, self.generations[index as usize])
            }
            None => {
                let index = u32::try_from(self.generations.len()).expect("entity index overflow");
                self.generations.push(0);
                self.alive.push(true);
                Entity::new(index, 0)
            }
        }
    }

    pub fn destroy(&mut self, entity: Entity) -> bool {
        if !self.is_alive(entity) {
            return false;
        }
        let slot = entity.index as usize;
        self.alive[slot] = false;
        self.generations[slot] = self.generations[slot].wrapping_add(1);
        self.free.push(entity.index);
        self.alive_count -= 1;
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        let slot = entity.index as usize;
        slot < self.generations.len()
            && self.alive[slot]
            && self.generations[slot] == entity.generation
    }

    pub fn live_at(&self, index: u32) -> Option<Entity> {
        let slot = index as usize;
        if slot < self.alive.len() && self.alive[slot] {
            Some(Entity::new(index, self.generations[slot]))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.alive_count
    }

    pub fn is_empty(&self) -> bool {
        self.alive_count == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        (0..self.generations.len() as u32).filter_map(|index| self.live_at(index))
    }

    pub fn hash_into(&self, hasher: &mut StateHasher) {
        hasher.write_u64(self.generations.len() as u64);
        for slot in 0..self.generations.len() {
            hasher.write_u32(self.generations[slot]);
            hasher.write_bool(self.alive[slot]);
        }
        hasher.write_u64(self.free.len() as u64);
        for &index in &self.free {
            hasher.write_u32(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_entities_get_sequential_indices_at_generation_zero() {
        let mut allocator = EntityAllocator::new();
        let a = allocator.create();
        let b = allocator.create();
        assert_eq!((a.index(), a.generation()), (0, 0));
        assert_eq!((b.index(), b.generation()), (1, 0));
        assert_eq!(allocator.len(), 2);
    }

    #[test]
    fn destroy_bumps_generation_and_rejects_stale_handles() {
        let mut allocator = EntityAllocator::new();
        let a = allocator.create();
        assert!(allocator.destroy(a));
        assert!(!allocator.is_alive(a));
        assert!(!allocator.destroy(a));
        let reused = allocator.create();
        assert_eq!(reused.index(), a.index());
        assert_eq!(reused.generation(), 1);
        assert!(allocator.is_alive(reused));
        assert!(!allocator.is_alive(a));
    }

    #[test]
    fn freed_indices_are_reused_last_in_first_out() {
        let mut allocator = EntityAllocator::new();
        let e: Vec<Entity> = (0..4).map(|_| allocator.create()).collect();
        allocator.destroy(e[1]);
        allocator.destroy(e[3]);
        assert_eq!(allocator.create().index(), 3);
        assert_eq!(allocator.create().index(), 1);
        assert_eq!(allocator.create().index(), 4);
    }

    #[test]
    fn iteration_is_in_index_order_and_skips_dead() {
        let mut allocator = EntityAllocator::new();
        let e: Vec<Entity> = (0..4).map(|_| allocator.create()).collect();
        allocator.destroy(e[1]);
        let indices: Vec<u32> = allocator.iter().map(Entity::index).collect();
        assert_eq!(indices, vec![0, 2, 3]);
        assert_eq!(allocator.len(), 3);
    }

    #[test]
    fn unknown_entity_is_not_alive() {
        let allocator = EntityAllocator::new();
        assert!(!allocator.is_alive(Entity::new(5, 0)));
        assert!(allocator.live_at(5).is_none());
    }

    #[test]
    fn hash_covers_generations_and_free_list() {
        let mut a = EntityAllocator::new();
        let mut b = EntityAllocator::new();
        let ea = a.create();
        b.create();
        let mut before = StateHasher::new();
        a.hash_into(&mut before);
        a.destroy(ea);
        a.create();
        let mut after = StateHasher::new();
        a.hash_into(&mut after);
        let mut plain = StateHasher::new();
        b.hash_into(&mut plain);
        assert_ne!(after.finish(), plain.finish());
        assert_eq!(before.finish(), plain.finish());
    }
}
