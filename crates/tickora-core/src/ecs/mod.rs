mod entity;
mod store;
mod world;

pub use entity::{Entity, EntityAllocator};
pub use store::SparseSet;
pub use world::{EcsError, World};

use crate::fx::Fx;
use crate::hash::StateHasher;

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    Int(i64),
    Bool(bool),
    Fx(Fx),
    Text(String),
}

pub type Fields = Vec<(&'static str, FieldValue)>;

pub trait Component: 'static {
    const NAME: &'static str;

    fn hash_into(&self, hasher: &mut StateHasher);

    fn fields(&self) -> Fields;
}
