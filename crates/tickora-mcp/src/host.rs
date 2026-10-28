use std::io::Write;

use serde_json::Value;
use tickora_audio::{Block, Capture, Mixer, wav};
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

    fn replay_audio(
        &self,
        to: u64,
        sink: &mut dyn FnMut(u64, &Block, &Mixer) -> Result<(), String>,
    ) -> Result<(), String>;
}

pub fn write_wav<H: Host + ?Sized>(
    host: &H,
    from: u64,
    to: u64,
    out: &mut dyn Write,
) -> Result<(), String> {
    let format = host.audio().format();
    host.audio().range(from, to)?;
    let frames = (to - from) * u64::from(format.samples_per_tick());
    out.write_all(&wav::header(format.sample_rate(), frames)?)
        .map_err(|e| e.to_string())?;
    host.replay_audio(to, &mut |tick, block, _| {
        if tick < from {
            return Ok(());
        }
        wav::write_frames(out, &block.frames).map_err(|e| e.to_string())
    })
}
