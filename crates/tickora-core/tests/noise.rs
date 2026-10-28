use tickora_core::ecs::{Entity, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::input::{InputCodec, InputEvent, InputLog};
use tickora_core::noise::{self, HeardNoise, Hearing, Location, Noise};
use tickora_core::rng::Rng;
use tickora_core::sim::{Runner, Sim};

const PINNED_REPLAY_HASH: u64 = 1636647955146064675;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gait {
    Still,
    Walk,
    Run,
}

impl InputCodec for Gait {
    fn encode(&self, out: &mut Vec<u8>) {
        out.push(*self as u8);
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0] => Some(Gait::Still),
            [1] => Some(Gait::Walk),
            [2] => Some(Gait::Run),
            _ => None,
        }
    }
}

impl Gait {
    fn speed_cm(self) -> i32 {
        match self {
            Gait::Still => 0,
            Gait::Walk => 3,
            Gait::Run => 8,
        }
    }

    fn loudness(self) -> Fx {
        match self {
            Gait::Still => Fx::ZERO,
            Gait::Walk => Fx::from_ratio(1, 4),
            Gait::Run => Fx::ONE,
        }
    }
}

struct Mine {
    world: World,
    miner: Entity,
    monster: Entity,
    gait: Gait,
}

impl Mine {
    fn new() -> Self {
        let mut world = World::new();
        noise::register(&mut world);
        let miner = world.spawn();
        world.insert(miner, Location::default()).unwrap();
        let monster = world.spawn();
        world
            .insert(
                monster,
                Location::new(Fx::from_int(1500), Fx::from_int(200), Fx::ZERO),
            )
            .unwrap();
        world
            .insert(
                monster,
                Hearing {
                    range: Fx::from_int(1000),
                },
            )
            .unwrap();
        Mine {
            world,
            miner,
            monster,
            gait: Gait::Still,
        }
    }
}

impl Sim for Mine {
    type Input = Gait;

    fn step(&mut self, tick: u64, inputs: &[InputEvent<Gait>], rng: &mut Rng) {
        noise::clear(&mut self.world);
        if let Some(event) = inputs.last() {
            self.gait = event.input;
        }
        let location = {
            let location = self.world.get_mut::<Location>(self.miner).unwrap();
            location.x += Fx::from_int(self.gait.speed_cm());
            *location
        };
        if self.gait != Gait::Still {
            let scuff = Fx::from_ratio(i64::from(rng.below(5)), 20);
            let mut step = Noise::new(location, self.gait.loudness() + scuff);
            step.instigator = Some(self.miner);
            noise::emit(&mut self.world, step);
        }
        noise::hear(&mut self.world, tick);
    }

    fn hash_state(&self, hasher: &mut StateHasher) {
        self.world.hash_into(hasher);
    }
}

fn log_from(seed: u64, ticks: u64) -> InputLog<Gait> {
    let mut rng = Rng::from_seed(seed);
    let mut log = InputLog::new();
    for tick in 0..ticks {
        if rng.chance(1, 20) {
            let gait = match rng.below(3) {
                0 => Gait::Still,
                1 => Gait::Walk,
                _ => Gait::Run,
            };
            log.push(tick, gait);
        }
    }
    log
}

#[test]
fn replay_of_a_noise_sim_is_identical_for_the_same_seed_and_log() {
    let log = log_from(0xC0FFEE, 600);
    let mut a = Runner::new(Mine::new(), 0xC0FFEE);
    let mut b = Runner::new(Mine::new(), 0xC0FFEE);
    let decoded = InputLog::<Gait>::from_bytes(&log.to_bytes()).unwrap();
    assert_eq!(a.run(&log, 600), b.run(&decoded, 600));
}

#[test]
fn replay_hash_is_pinned() {
    let log = log_from(0xC0FFEE, 600);
    let mut runner = Runner::new(Mine::new(), 0xC0FFEE);
    assert_eq!(runner.run(&log, 600).hash, PINNED_REPLAY_HASH);
    let mine = runner.sim();
    assert!(mine.world.get::<HeardNoise>(mine.monster).is_some());
}

#[test]
fn running_is_heard_from_further_away_than_walking() {
    let walk_ticks = 300;
    let run_ticks = 120;
    let mut log = InputLog::new();
    log.push(0, Gait::Walk);
    let mut walker = Runner::new(Mine::new(), 1);
    walker.run(&log, walk_ticks);
    let mine = walker.sim();
    assert!(mine.world.get::<HeardNoise>(mine.monster).is_none());

    let mut log = InputLog::new();
    log.push(0, Gait::Run);
    let mut runner = Runner::new(Mine::new(), 1);
    runner.run(&log, run_ticks);
    let mine = runner.sim();
    let heard = mine.world.get::<HeardNoise>(mine.monster).unwrap();
    assert_eq!(heard.instigator, Some(mine.miner));
    assert_eq!(heard.tick, run_ticks - 1);
    assert_eq!(mine.world.query::<Noise>().len(), 1);
}
