mod entity;
mod store;
mod world;

pub use entity::{Entity, EntityAllocator};
pub use store::SparseSet;
pub use world::{EcsError, World};

use crate::hash::StateHasher;

pub trait Component: 'static {
    const NAME: &'static str;

    fn hash_into(&self, hasher: &mut StateHasher);
}
