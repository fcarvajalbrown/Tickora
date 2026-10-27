use crate::hash::StateHasher;
use crate::input::{InputEvent, InputLog};
use crate::rng::Rng;

pub trait Sim {
    type Input;

    fn step(&mut self, tick: u64, inputs: &[InputEvent<Self::Input>], rng: &mut Rng);

    fn hash_state(&self, hasher: &mut StateHasher);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayHash {
    pub ticks: u64,
    pub hash: u64,
}

pub struct Runner<S: Sim> {
    sim: S,
    rng: Rng,
    tick: u64,
    chain: StateHasher,
}

impl<S: Sim> Runner<S> {
    pub fn new(sim: S, seed: u64) -> Self {
        let mut runner = Self {
            sim,
            rng: Rng::from_seed(seed),
            tick: 0,
            chain: StateHasher::new(),
        };
        runner.record_tick_hash();
        runner
    }

    pub fn sim(&self) -> &S {
        &self.sim
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn replay_hash(&self) -> ReplayHash {
        ReplayHash {
            ticks: self.tick,
            hash: self.chain.finish(),
        }
    }

    pub fn state_hash(&self) -> u64 {
        self.snapshot_hasher().finish()
    }

    pub fn step(&mut self, log: &InputLog<S::Input>) -> u64 {
        self.sim.step(self.tick, log.at(self.tick), &mut self.rng);
        self.tick += 1;
        self.record_tick_hash()
    }

    pub fn run(&mut self, log: &InputLog<S::Input>, ticks: u64) -> ReplayHash {
        self.run_with(log, ticks, |_, _| {})
    }

    pub fn run_with(
        &mut self,
        log: &InputLog<S::Input>,
        ticks: u64,
        mut on_tick: impl FnMut(u64, u64),
    ) -> ReplayHash {
        for _ in 0..ticks {
            let state_hash = self.step(log);
            on_tick(self.tick, state_hash);
        }
        self.replay_hash()
    }

    fn snapshot_hasher(&self) -> StateHasher {
        let mut hasher = StateHasher::new();
        let (state, increment) = self.rng.raw_parts();
        hasher.write_u64(state);
        hasher.write_u64(increment);
        self.sim.hash_state(&mut hasher);
        hasher
    }

    fn record_tick_hash(&mut self) -> u64 {
        let state_hash = self.snapshot_hasher().finish();
        self.chain.write_u64(self.tick);
        self.chain.write_u64(state_hash);
        state_hash
    }
}

pub fn first_divergence(a: &[u64], b: &[u64]) -> Option<usize> {
    let shared = a.len().min(b.len());
    (0..shared)
        .find(|&index| a[index] != b[index])
        .or((a.len() != b.len()).then_some(shared))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_divergence_finds_index_and_length_mismatch() {
        assert_eq!(first_divergence(&[1, 2, 3], &[1, 2, 3]), None);
        assert_eq!(first_divergence(&[1, 2, 3], &[1, 9, 3]), Some(1));
        assert_eq!(first_divergence(&[1, 2], &[1, 2, 3]), Some(2));
    }
}
