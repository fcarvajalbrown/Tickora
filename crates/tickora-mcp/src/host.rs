use std::io::Write;

use serde_json::Value;
use tickora_audio::Capture;
use tickora_core::ecs::World;
use tickora_render::{Scene, Scene3, ViewMode};

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

    fn scene(&self, view_mode: ViewMode) -> Scene;

    fn scene3(&self) -> Scene3;

    fn audio(&self) -> &Capture;

    fn write_wav(&self, from: u64, to: u64, out: &mut dyn Write) -> Result<(), String>;
}
