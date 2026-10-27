use tickora_core::hash::StateHasher;
use tickora_core::input::{InputCodec, InputEvent, InputLog};
use tickora_core::rng::Rng;
use tickora_core::sim::{ReplayHash, Runner, Sim, first_divergence};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Nudge { dx: i32, dy: i32 },
    Spawn,
    Reset,
}

impl InputCodec for Command {
    fn encode(&self, out: &mut Vec<u8>) {
        match *self {
            Command::Nudge { dx, dy } => {
                out.push(0);
                out.extend_from_slice(&dx.to_le_bytes());
                out.extend_from_slice(&dy.to_le_bytes());
            }
            Command::Spawn => out.push(1),
            Command::Reset => out.push(2),
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0, rest @ ..] if rest.len() == 8 => Some(Command::Nudge {
                dx: i32::from_le_bytes(rest[0..4].try_into().ok()?),
                dy: i32::from_le_bytes(rest[4..8].try_into().ok()?),
            }),
            [1] => Some(Command::Spawn),
            [2] => Some(Command::Reset),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Body {
    x: i64,
    y: i64,
    vx: i32,
    vy: i32,
}

#[derive(Default)]
struct World {
    bodies: Vec<Body>,
    score: u64,
}

impl Sim for World {
    type Input = Command;

    fn step(&mut self, tick: u64, inputs: &[InputEvent<Command>], rng: &mut Rng) {
        for event in inputs {
            match event.input {
                Command::Nudge { dx, dy } => {
                    for body in &mut self.bodies {
                        body.vx = body.vx.wrapping_add(dx);
                        body.vy = body.vy.wrapping_add(dy);
                    }
                }
                Command::Spawn => self.bodies.push(Body {
                    x: i64::from(rng.below(1000)),
                    y: i64::from(rng.below(1000)),
                    vx: rng.below(21) as i32 - 10,
                    vy: rng.below(21) as i32 - 10,
                }),
                Command::Reset => self.bodies.clear(),
            }
        }
        for body in &mut self.bodies {
            body.x += i64::from(body.vx);
            body.y += i64::from(body.vy);
            if rng.chance(1, 8) {
                body.vx = -body.vx;
            }
        }
        self.score = self.score.wrapping_add(tick ^ self.bodies.len() as u64);
    }

    fn hash_state(&self, hasher: &mut StateHasher) {
        hasher.write_u64(self.score);
        hasher.write_u64(self.bodies.len() as u64);
        for body in &self.bodies {
            hasher.write_i64(body.x);
            hasher.write_i64(body.y);
            hasher.write_i32(body.vx);
            hasher.write_i32(body.vy);
        }
    }
}

fn scripted_log() -> InputLog<Command> {
    let mut log = InputLog::new();
    for tick in (0..600).step_by(7) {
        log.push(tick, Command::Spawn);
    }
    for tick in (3..600).step_by(11) {
        log.push(
            tick,
            Command::Nudge {
                dx: (tick % 5) as i32 - 2,
                dy: (tick % 3) as i32 - 1,
            },
        );
    }
    log.push(450, Command::Reset);
    log.push(450, Command::Spawn);
    log
}

fn replay(seed: u64, log: &InputLog<Command>, ticks: u64) -> ReplayHash {
    Runner::new(World::default(), seed).run(log, ticks)
}

fn fuzzed_log(seed: u64) -> InputLog<Command> {
    let mut rng = Rng::from_seed(seed);
    let mut log = InputLog::new();
    for _ in 0..rng.below(200) {
        let tick = u64::from(rng.below(300));
        let command = match rng.below(3) {
            0 => Command::Nudge {
                dx: rng.next_u32() as i32,
                dy: rng.next_u32() as i32,
            },
            1 => Command::Spawn,
            _ => Command::Reset,
        };
        log.push(tick, command);
    }
    log
}

#[test]
fn same_seed_and_inputs_give_identical_hash() {
    let log = scripted_log();
    assert_eq!(replay(1234, &log, 600), replay(1234, &log, 600));
}

#[test]
fn different_seed_changes_hash() {
    let log = scripted_log();
    assert_ne!(replay(1, &log, 600).hash, replay(2, &log, 600).hash);
}

#[test]
fn different_inputs_change_hash() {
    let log = scripted_log();
    let mut altered = scripted_log();
    altered.push(300, Command::Spawn);
    assert_ne!(replay(1, &log, 600).hash, replay(1, &altered, 600).hash);
}

#[test]
fn replay_length_is_part_of_the_hash() {
    let log = scripted_log();
    assert_ne!(replay(1, &log, 599), replay(1, &log, 600));
}

#[test]
fn serialized_log_replays_to_the_same_hash() {
    let log = scripted_log();
    let restored = InputLog::<Command>::from_bytes(&log.to_bytes()).unwrap();
    assert_eq!(replay(77, &log, 600), replay(77, &restored, 600));
}

#[test]
fn splitting_a_run_into_segments_changes_nothing() {
    let log = scripted_log();
    let whole = replay(5, &log, 600);
    let mut runner = Runner::new(World::default(), 5);
    runner.run(&log, 100);
    runner.run(&log, 250);
    runner.run(&log, 250);
    assert_eq!(runner.replay_hash(), whole);
}

#[test]
fn per_tick_trace_localises_a_divergence() {
    let log = scripted_log();
    let mut altered = scripted_log();
    altered.push(300, Command::Spawn);

    let trace = |log: &InputLog<Command>| {
        let mut hashes = Vec::new();
        Runner::new(World::default(), 9).run_with(log, 600, |_, hash| hashes.push(hash));
        hashes
    };

    assert_eq!(first_divergence(&trace(&log), &trace(&log)), None);
    assert_eq!(first_divergence(&trace(&log), &trace(&altered)), Some(300));
}

#[test]
fn fuzzed_logs_replay_identically_and_survive_serialization() {
    for seed in 0..300u64 {
        let log = fuzzed_log(seed);
        let first = replay(seed, &log, 300);
        assert_eq!(first, replay(seed, &log, 300), "seed {seed}");
        let restored = InputLog::<Command>::from_bytes(&log.to_bytes()).unwrap();
        assert_eq!(restored, log, "seed {seed}");
        assert_eq!(first, replay(seed, &restored, 300), "seed {seed}");
    }
}

#[test]
fn golden_hash_is_pinned_across_platforms() {
    let log = scripted_log();
    let result = replay(0xC0FFEE, &log, 600);
    assert_eq!(result.ticks, 600);
    assert_eq!(result.hash, 238_730_705_328_002_493);
}
