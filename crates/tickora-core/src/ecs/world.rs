use super::entity::{Entity, EntityAllocator};
use super::store::{ErasedStore, SparseSet};
use super::{Component, Fields};
use crate::hash::StateHasher;
use core::any::TypeId;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EcsError {
    DeadEntity(Entity),
    NotRegistered(&'static str),
    UnknownComponent(String),
}

impl fmt::Display for EcsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EcsError::DeadEntity(entity) => write!(
                f,
                "entity {}v{} is not alive",
                entity.index(),
                entity.generation()
            ),
            EcsError::NotRegistered(name) => write!(f, "component {name} is not registered"),
            EcsError::UnknownComponent(name) => write!(f, "unknown component {name}"),
        }
    }
}

impl std::error::Error for EcsError {}

struct Registered {
    type_id: TypeId,
    store: Box<dyn ErasedStore>,
}

#[derive(Default)]
pub struct World {
    entities: EntityAllocator,
    stores: Vec<Registered>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Component>(&mut self) {
        if self.slot::<T>().is_none() {
            self.stores.push(Registered {
                type_id: TypeId::of::<T>(),
                store: Box::new(SparseSet::<T>::new()),
            });
        }
    }

    pub fn component_names(&self) -> Vec<&'static str> {
        self.stores.iter().map(|r| r.store.name()).collect()
    }

    pub fn component_count(&self, name: &str) -> Option<usize> {
        self.stores
            .iter()
            .find(|r| r.store.name() == name)
            .map(|r| r.store.len())
    }

    pub fn spawn(&mut self) -> Entity {
        self.entities.create()
    }

    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.destroy(entity) {
            return false;
        }
        for registered in &mut self.stores {
            registered.store.remove_entity(entity.index());
        }
        true
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter()
    }

    pub fn insert<T: Component>(
        &mut self,
        entity: Entity,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        if !self.entities.is_alive(entity) {
            return Err(EcsError::DeadEntity(entity));
        }
        let store = self
            .store_mut::<T>()
            .ok_or(EcsError::NotRegistered(T::NAME))?;
        Ok(store.insert(entity.index(), value))
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) -> Result<Option<T>, EcsError> {
        if !self.entities.is_alive(entity) {
            return Err(EcsError::DeadEntity(entity));
        }
        let store = self
            .store_mut::<T>()
            .ok_or(EcsError::NotRegistered(T::NAME))?;
        Ok(store.remove(entity.index()))
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        self.store::<T>()?.get(entity.index())
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        self.store_mut::<T>()?.get_mut(entity.index())
    }

    pub fn has<T: Component>(&self, entity: Entity) -> bool {
        self.get::<T>(entity).is_some()
    }

    pub fn query<T: Component>(&self) -> Vec<(Entity, &T)> {
        let Some(store) = self.store::<T>() else {
            return Vec::new();
        };
        store
            .iter()
            .filter_map(|(index, value)| Some((self.entities.live_at(index)?, value)))
            .collect()
    }

    pub fn query_mut<T: Component>(&mut self) -> Vec<(Entity, &mut T)> {
        let entities = &self.entities;
        let Some(registered) = self
            .stores
            .iter_mut()
            .find(|r| r.type_id == TypeId::of::<T>())
        else {
            return Vec::new();
        };
        let store = registered
            .store
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()
            .expect("store type matches its TypeId");
        store
            .iter_mut()
            .filter_map(|(index, value)| Some((entities.live_at(index)?, value)))
            .collect()
    }

    pub fn query2<A: Component, B: Component>(&self) -> Vec<(Entity, &A, &B)> {
        let (Some(a), Some(b)) = (self.store::<A>(), self.store::<B>()) else {
            return Vec::new();
        };
        a.iter()
            .filter_map(|(index, left)| {
                let right = b.get(index)?;
                Some((self.entities.live_at(index)?, left, right))
            })
            .collect()
    }

    pub fn describe(&self, entity: Entity) -> Option<Vec<(&'static str, Fields)>> {
        if !self.entities.is_alive(entity) {
            return None;
        }
        Some(
            self.stores
                .iter()
                .filter_map(|r| Some((r.store.name(), r.store.fields_of(entity.index())?)))
                .collect(),
        )
    }

    pub fn query_names(&self, names: &[&str]) -> Result<Vec<Entity>, EcsError> {
        let mut stores = Vec::with_capacity(names.len());
        for name in names {
            let registered = self
                .stores
                .iter()
                .find(|r| r.store.name() == *name)
                .ok_or_else(|| EcsError::UnknownComponent((*name).to_string()))?;
            stores.push(&registered.store);
        }
        let Some(smallest) = stores.iter().min_by_key(|store| store.len()) else {
            return Ok(self.entities.iter().collect());
        };
        Ok(smallest
            .indices()
            .into_iter()
            .filter(|&index| stores.iter().all(|store| store.has(index)))
            .filter_map(|index| self.entities.live_at(index))
            .collect())
    }

    pub fn hash_into(&self, hasher: &mut StateHasher) {
        self.entities.hash_into(hasher);
        hasher.write_u64(self.stores.len() as u64);
        for registered in &self.stores {
            registered.store.hash_into(hasher);
        }
    }

    pub fn state_hash(&self) -> u64 {
        let mut hasher = StateHasher::new();
        self.hash_into(&mut hasher);
        hasher.finish()
    }

    fn slot<T: Component>(&self) -> Option<usize> {
        let wanted = TypeId::of::<T>();
        self.stores.iter().position(|r| r.type_id == wanted)
    }

    fn store<T: Component>(&self) -> Option<&SparseSet<T>> {
        let slot = self.slot::<T>()?;
        self.stores[slot].store.as_any().downcast_ref()
    }

    fn store_mut<T: Component>(&mut self) -> Option<&mut SparseSet<T>> {
        let slot = self.slot::<T>()?;
        self.stores[slot].store.as_any_mut().downcast_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::FieldValue;
    use crate::fx::Fx;

    #[derive(Clone, Debug, PartialEq)]
    struct Position {
        x: Fx,
        y: Fx,
    }

    impl Component for Position {
        const NAME: &'static str = "position";

        fn hash_into(&self, hasher: &mut StateHasher) {
            self.x.hash_into(hasher);
            self.y.hash_into(hasher);
        }

        fn fields(&self) -> Fields {
            vec![("x", FieldValue::Fx(self.x)), ("y", FieldValue::Fx(self.y))]
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Health(i32);

    impl Component for Health {
        const NAME: &'static str = "health";

        fn hash_into(&self, hasher: &mut StateHasher) {
            hasher.write_i32(self.0);
        }

        fn fields(&self) -> Fields {
            vec![("hp", FieldValue::Int(i64::from(self.0)))]
        }
    }

    fn pos(x: i32, y: i32) -> Position {
        Position {
            x: Fx::from_int(x),
            y: Fx::from_int(y),
        }
    }

    fn world() -> World {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Health>();
        world
    }

    #[test]
    fn insert_get_replace_remove() {
        let mut world = world();
        let e = world.spawn();
        assert_eq!(world.insert(e, Health(10)), Ok(None));
        assert_eq!(world.insert(e, Health(7)), Ok(Some(Health(10))));
        assert_eq!(world.get::<Health>(e), Some(&Health(7)));
        assert!(world.has::<Health>(e));
        assert!(!world.has::<Position>(e));
        assert_eq!(world.remove::<Health>(e), Ok(Some(Health(7))));
        assert_eq!(world.get::<Health>(e), None);
    }

    #[test]
    fn get_mut_edits_in_place() {
        let mut world = world();
        let e = world.spawn();
        world.insert(e, Health(1)).unwrap();
        world.get_mut::<Health>(e).unwrap().0 = 99;
        assert_eq!(world.get::<Health>(e), Some(&Health(99)));
    }

    #[test]
    fn unregistered_component_is_an_error_on_write_and_none_on_read() {
        let mut world = World::new();
        let e = world.spawn();
        assert_eq!(
            world.insert(e, Health(1)),
            Err(EcsError::NotRegistered("health"))
        );
        assert_eq!(world.get::<Health>(e), None);
        assert!(world.query::<Health>().is_empty());
    }

    #[test]
    fn dead_entity_is_rejected() {
        let mut world = world();
        let e = world.spawn();
        assert!(world.despawn(e));
        assert_eq!(world.insert(e, Health(1)), Err(EcsError::DeadEntity(e)));
        assert_eq!(world.remove::<Health>(e), Err(EcsError::DeadEntity(e)));
        assert!(!world.despawn(e));
    }

    #[test]
    fn despawn_clears_components_and_stale_handle_sees_nothing() {
        let mut world = world();
        let old = world.spawn();
        world.insert(old, Health(5)).unwrap();
        world.despawn(old);
        let fresh = world.spawn();
        assert_eq!(fresh.index(), old.index());
        assert_eq!(world.get::<Health>(old), None);
        assert_eq!(world.get::<Health>(fresh), None);
        assert_eq!(world.component_count("health"), Some(0));
    }

    #[test]
    fn query_returns_dense_order_with_entities() {
        let mut world = world();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        for (e, hp) in [(c, 3), (a, 1), (b, 2)] {
            world.insert(e, Health(hp)).unwrap();
        }
        let order: Vec<(Entity, i32)> = world
            .query::<Health>()
            .into_iter()
            .map(|(e, h)| (e, h.0))
            .collect();
        assert_eq!(order, vec![(c, 3), (a, 1), (b, 2)]);
    }

    #[test]
    fn query_mut_edits_every_match() {
        let mut world = world();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Health(1)).unwrap();
        world.insert(b, Health(2)).unwrap();
        for (_, health) in world.query_mut::<Health>() {
            health.0 += 100;
        }
        assert_eq!(world.get::<Health>(a), Some(&Health(101)));
        assert_eq!(world.get::<Health>(b), Some(&Health(102)));
    }

    #[test]
    fn query2_yields_only_entities_with_both() {
        let mut world = world();
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        world.insert(a, pos(1, 1)).unwrap();
        world.insert(a, Health(1)).unwrap();
        world.insert(b, pos(2, 2)).unwrap();
        world.insert(c, Health(3)).unwrap();
        world.insert(c, pos(3, 3)).unwrap();
        let hits: Vec<Entity> = world
            .query2::<Position, Health>()
            .into_iter()
            .map(|(e, _, _)| e)
            .collect();
        assert_eq!(hits, vec![a, c]);
    }

    #[test]
    fn describe_lists_present_components_in_registration_order() {
        let mut world = world();
        let e = world.spawn();
        world.insert(e, Health(4)).unwrap();
        world.insert(e, pos(1, 2)).unwrap();
        let described = world.describe(e).unwrap();
        let names: Vec<&str> = described.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, vec!["position", "health"]);
        assert_eq!(described[1].1, vec![("hp", FieldValue::Int(4))]);
        world.despawn(e);
        assert!(world.describe(e).is_none());
    }

    #[test]
    fn query_names_intersects_and_rejects_unknown_names() {
        let mut world = world();
        let a = world.spawn();
        let b = world.spawn();
        world.insert(a, Health(1)).unwrap();
        world.insert(a, pos(0, 0)).unwrap();
        world.insert(b, Health(2)).unwrap();
        assert_eq!(world.query_names(&["health", "position"]), Ok(vec![a]));
        assert_eq!(world.query_names(&["health"]), Ok(vec![a, b]));
        assert_eq!(world.query_names(&[]), Ok(vec![a, b]));
        assert_eq!(
            world.query_names(&["ghost"]),
            Err(EcsError::UnknownComponent("ghost".to_string()))
        );
    }

    #[test]
    fn registration_is_idempotent_and_ordered() {
        let mut world = World::new();
        world.register::<Health>();
        world.register::<Position>();
        world.register::<Health>();
        assert_eq!(world.component_names(), vec!["health", "position"]);
    }

    #[test]
    fn hash_is_stable_for_identical_operations() {
        let build = || {
            let mut world = world();
            let a = world.spawn();
            let b = world.spawn();
            world.insert(a, pos(3, 4)).unwrap();
            world.insert(b, Health(8)).unwrap();
            world
        };
        assert_eq!(build().state_hash(), build().state_hash());
    }

    #[test]
    fn hash_changes_with_value_registration_order_and_generation() {
        let base = {
            let mut world = world();
            let a = world.spawn();
            world.insert(a, Health(8)).unwrap();
            world
        };

        let mut changed = world();
        let a = changed.spawn();
        changed.insert(a, Health(9)).unwrap();
        assert_ne!(base.state_hash(), changed.state_hash());

        let mut reordered = World::new();
        reordered.register::<Health>();
        reordered.register::<Position>();
        let a = reordered.spawn();
        reordered.insert(a, Health(8)).unwrap();
        assert_ne!(base.state_hash(), reordered.state_hash());

        let mut recycled = world();
        let first = recycled.spawn();
        recycled.despawn(first);
        let second = recycled.spawn();
        recycled.insert(second, Health(8)).unwrap();
        assert_ne!(base.state_hash(), recycled.state_hash());
    }
}
