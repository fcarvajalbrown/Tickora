use std::collections::BTreeMap;
use tickora_core::ecs::{Component, Entity, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::input::{InputCodec, InputEvent, InputLog};
use tickora_core::rng::Rng;
use tickora_core::sim::{Runner, Sim, first_divergence};

#[derive(Clone, Debug, PartialEq)]
struct Position {
    x: Fx,
    y: Fx,
}

#[derive(Clone, Debug, PartialEq)]
struct Velocity {
    x: Fx,
    y: Fx,
}

#[derive(Clone, Debug, PartialEq)]
struct Health(i32);

impl Component for Position {
    const NAME: &'static str = "position";

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.x.hash_into(hasher);
        self.y.hash_into(hasher);
    }
}

impl Component for Velocity {
    const NAME: &'static str = "velocity";

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.x.hash_into(hasher);
        self.y.hash_into(hasher);
    }
}

impl Component for Health {
    const NAME: &'static str = "health";

    fn hash_into(&self, hasher: &mut StateHasher) {
        hasher.write_i32(self.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Spawn,
    Damage(u32),
}

impl InputCodec for Command {
    fn encode(&self, out: &mut Vec<u8>) {
        match *self {
            Command::Spawn => out.push(0),
            Command::Damage(amount) => {
                out.push(1);
                out.extend_from_slice(&amount.to_le_bytes());
            }
        }
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0] => Some(Command::Spawn),
            [1, rest @ ..] if rest.len() == 4 => {
                Some(Command::Damage(u32::from_le_bytes(rest.try_into().ok()?)))
            }
            _ => None,
        }
    }
}

struct Arena {
    world: World,
}

impl Arena {
    fn new() -> Self {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Health>();
        Arena { world }
    }
}

impl Sim for Arena {
    type Input = Command;

    fn step(&mut self, _tick: u64, inputs: &[InputEvent<Command>], rng: &mut Rng) {
        for event in inputs {
            match event.input {
                Command::Spawn => {
                    let e = self.world.spawn();
                    let spawn = |rng: &mut Rng| Fx::from_int(rng.below(100) as i32 - 50);
                    let (px, py) = (spawn(rng), spawn(rng));
                    let (vx, vy) = (spawn(rng) / Fx::from_int(10), spawn(rng) / Fx::from_int(10));
                    self.world.insert(e, Position { x: px, y: py }).unwrap();
                    self.world.insert(e, Velocity { x: vx, y: vy }).unwrap();
                    self.world.insert(e, Health(100)).unwrap();
                }
                Command::Damage(amount) => {
                    let victims: Vec<Entity> = self
                        .world
                        .query::<Health>()
                        .into_iter()
                        .map(|(e, _)| e)
                        .collect();
                    if victims.is_empty() {
                        continue;
                    }
                    let target = victims[(amount as usize) % victims.len()];
                    let dead = {
                        let health = self.world.get_mut::<Health>(target).unwrap();
                        health.0 -= amount as i32 % 60;
                        health.0 <= 0
                    };
                    if dead {
                        self.world.despawn(target);
                    }
                }
            }
        }
        let movers: Vec<(Entity, Velocity)> = self
            .world
            .query::<Velocity>()
            .into_iter()
            .map(|(e, v)| (e, v.clone()))
            .collect();
        for (entity, velocity) in movers {
            if let Some(position) = self.world.get_mut::<Position>(entity) {
                position.x += velocity.x;
                position.y += velocity.y;
            }
        }
    }

    fn hash_state(&self, hasher: &mut StateHasher) {
        self.world.hash_into(hasher);
    }
}

fn log_from(seed: u64, ticks: u64) -> InputLog<Command> {
    let mut rng = Rng::from_seed(seed);
    let mut log = InputLog::new();
    for tick in 0..ticks {
        if rng.chance(1, 3) {
            log.push(tick, Command::Spawn);
        }
        if rng.chance(1, 4) {
            log.push(tick, Command::Damage(rng.next_u32()));
        }
    }
    log
}

#[test]
fn replay_of_an_ecs_sim_is_identical_for_the_same_seed_and_log() {
    let log = log_from(7, 400);
    let mut a = Runner::new(Arena::new(), 99);
    let mut b = Runner::new(Arena::new(), 99);
    assert_eq!(a.run(&log, 400), b.run(&log, 400));
}

#[test]
fn replay_survives_an_encode_decode_round_trip_of_the_log() {
    let log = log_from(11, 300);
    let decoded = InputLog::<Command>::from_bytes(&log.to_bytes()).unwrap();
    let mut a = Runner::new(Arena::new(), 5);
    let mut b = Runner::new(Arena::new(), 5);
    assert_eq!(a.run(&log, 300), b.run(&decoded, 300));
}

#[test]
fn different_seeds_diverge() {
    let log = log_from(3, 200);
    let mut a = Runner::new(Arena::new(), 1);
    let mut b = Runner::new(Arena::new(), 2);
    let mut hashes_a = Vec::new();
    let mut hashes_b = Vec::new();
    a.run_with(&log, 200, |_, hash| hashes_a.push(hash));
    b.run_with(&log, 200, |_, hash| hashes_b.push(hash));
    assert!(first_divergence(&hashes_a, &hashes_b).is_some());
}

#[test]
fn ecs_sim_replay_hash_is_pinned() {
    let log = log_from(0xC0FFEE, 600);
    let mut runner = Runner::new(Arena::new(), 0xC0FFEE);
    let replay = runner.run(&log, 600);
    assert_eq!(replay.hash, PINNED_HASH);
}

const PINNED_HASH: u64 = 9281879666421455162;

#[test]
fn world_matches_a_reference_model_under_random_operations() {
    let mut rng = Rng::from_seed(2024);
    let mut world = World::new();
    world.register::<Health>();
    world.register::<Position>();
    let mut handles: Vec<Entity> = Vec::new();
    let mut model_health: BTreeMap<Entity, i32> = BTreeMap::new();
    let mut model_position: BTreeMap<Entity, i32> = BTreeMap::new();

    for _ in 0..5000 {
        match rng.below(6) {
            0 => handles.push(world.spawn()),
            1 if !handles.is_empty() => {
                let e = handles[rng.below(handles.len() as u32) as usize];
                let was_alive = world.is_alive(e);
                assert_eq!(world.despawn(e), was_alive);
                model_health.remove(&e);
                model_position.remove(&e);
            }
            2 if !handles.is_empty() => {
                let e = handles[rng.below(handles.len() as u32) as usize];
                let value = rng.below(1000) as i32;
                if world.is_alive(e) {
                    world.insert(e, Health(value)).unwrap();
                    model_health.insert(e, value);
                } else {
                    assert!(world.insert(e, Health(value)).is_err());
                }
            }
            3 if !handles.is_empty() => {
                let e = handles[rng.below(handles.len() as u32) as usize];
                let value = rng.below(1000) as i32;
                if world.is_alive(e) {
                    world
                        .insert(
                            e,
                            Position {
                                x: Fx::from_int(value),
                                y: Fx::ZERO,
                            },
                        )
                        .unwrap();
                    model_position.insert(e, value);
                }
            }
            4 if !handles.is_empty() => {
                let e = handles[rng.below(handles.len() as u32) as usize];
                if world.is_alive(e) {
                    let removed = world.remove::<Health>(e).unwrap().map(|h| h.0);
                    assert_eq!(removed, model_health.remove(&e));
                }
            }
            _ => {}
        }
    }

    for &e in &handles {
        assert_eq!(
            world.get::<Health>(e).map(|h| h.0),
            model_health.get(&e).copied()
        );
        assert_eq!(
            world.get::<Position>(e).map(|p| p.x.floor_to_int() as i32),
            model_position.get(&e).copied()
        );
    }
    let mut queried: Vec<Entity> = world
        .query::<Health>()
        .into_iter()
        .map(|(e, _)| e)
        .collect();
    queried.sort();
    let expected: Vec<Entity> = model_health.keys().copied().collect();
    assert_eq!(queried, expected);
    let mut both: Vec<Entity> = world
        .query2::<Health, Position>()
        .into_iter()
        .map(|(e, _, _)| e)
        .collect();
    both.sort();
    let expected_both: Vec<Entity> = model_health
        .keys()
        .filter(|e| model_position.contains_key(e))
        .copied()
        .collect();
    assert_eq!(both, expected_both);
}
