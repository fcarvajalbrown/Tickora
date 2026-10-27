use serde_json::{Value, json};
use tickora_core::ecs::{Component, Entity, FieldValue, Fields, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::input::{InputEvent, InputLog};
use tickora_core::rng::Rng;
use tickora_core::sim::{Runner, Sim};

use crate::host::Host;

#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub x: Fx,
    pub y: Fx,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Velocity {
    pub x: Fx,
    pub y: Fx,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Health(pub i32);

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

impl Component for Velocity {
    const NAME: &'static str = "velocity";

    fn hash_into(&self, hasher: &mut StateHasher) {
        self.x.hash_into(hasher);
        self.y.hash_into(hasher);
    }

    fn fields(&self) -> Fields {
        vec![("x", FieldValue::Fx(self.x)), ("y", FieldValue::Fx(self.y))]
    }
}

impl Component for Health {
    const NAME: &'static str = "health";

    fn hash_into(&self, hasher: &mut StateHasher) {
        hasher.write_i32(self.0);
    }

    fn fields(&self) -> Fields {
        vec![("hp", FieldValue::Int(i64::from(self.0)))]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Spawn,
    Damage(u32),
}

impl Command {
    pub fn from_json(value: &Value) -> Result<Command, String> {
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("input {value} needs a string \"type\""))?;
        match kind {
            "spawn" => Ok(Command::Spawn),
            "damage" => value
                .get("amount")
                .and_then(Value::as_u64)
                .and_then(|amount| u32::try_from(amount).ok())
                .map(Command::Damage)
                .ok_or_else(|| {
                    format!("damage input {value} needs an \"amount\" from 0 to 4294967295")
                }),
            other => Err(format!(
                "unknown input type {other:?}; expected \"spawn\" or \"damage\""
            )),
        }
    }
}

pub struct Arena {
    world: World,
}

impl Arena {
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Health>();
        Arena { world }
    }

    pub fn world(&self) -> &World {
        &self.world
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
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

pub struct ArenaHost {
    seed: u64,
    runner: Runner<Arena>,
    log: InputLog<Command>,
}

impl ArenaHost {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            runner: Runner::new(Arena::new(), seed),
            log: InputLog::new(),
        }
    }

    pub fn log(&self) -> &InputLog<Command> {
        &self.log
    }
}

impl Host for ArenaHost {
    fn input_schema(&self) -> Value {
        json!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": { "type": { "const": "spawn" } },
                    "required": ["type"],
                    "additionalProperties": false,
                },
                {
                    "type": "object",
                    "properties": {
                        "type": { "const": "damage" },
                        "amount": { "type": "integer", "minimum": 0, "maximum": u32::MAX },
                    },
                    "required": ["type", "amount"],
                    "additionalProperties": false,
                },
            ],
        })
    }

    fn reset(&mut self, seed: u64) {
        *self = ArenaHost::new(seed);
    }

    fn step(&mut self, inputs: &[Value]) -> Result<(), String> {
        let commands = inputs
            .iter()
            .map(Command::from_json)
            .collect::<Result<Vec<_>, _>>()?;
        let tick = self.runner.tick();
        for command in commands {
            self.log.push(tick, command);
        }
        self.runner.step(&self.log);
        Ok(())
    }

    fn seed(&self) -> u64 {
        self.seed
    }

    fn tick(&self) -> u64 {
        self.runner.tick()
    }

    fn state_hash(&self) -> u64 {
        self.runner.state_hash()
    }

    fn replay_hash(&self) -> u64 {
        self.runner.replay_hash().hash
    }

    fn world(&self) -> &World {
        self.runner.sim().world()
    }
}
