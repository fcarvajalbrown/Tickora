use serde_json::Value;
use tickora_core::ecs::World;

pub trait Host {
    fn input_schema(&self) -> Value;

    fn fresh(&self, seed: u64) -> Self
    where
        Self: Sized;

    fn reset(&mut self, seed: u64);

    fn step(&mut self, inputs: &[Value]) -> Result<(), String>;

    fn seed(&self) -> u64;

    fn tick(&self) -> u64;

    fn state_hash(&self) -> u64;

    fn replay_hash(&self) -> u64;

    fn world(&self) -> &World;
}
