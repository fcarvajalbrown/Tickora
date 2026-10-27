use serde_json::{Value, json};
use tickora_core::ecs::{Component, Entity, FieldValue, Fields, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::input::{InputEvent, InputLog};
use tickora_core::rng::Rng;
use tickora_core::sim::{Runner, Sim};
use tickora_render::{Color, Grid, Quad, Scene, ViewMode};

use crate::host::Host;

pub const BACKGROUND: Color = [24, 24, 28, 255];
pub const GRID_LINE: Color = [48, 48, 56, 255];
pub const GRID_AXIS: Color = [96, 96, 110, 255];
pub const GRID_SPACING: f64 = 32.0;
pub const ENTITY_HALF_EXTENT: f64 = 2.0;

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

    pub fn scene(&self, view_mode: ViewMode) -> Scene {
        let quads = self
            .world
            .query2::<Position, Health>()
            .into_iter()
            .map(|(_, position, health)| Quad {
                center: [position.x.to_f64(), position.y.to_f64()],
                half_extent: ENTITY_HALF_EXTENT,
                color: hp_color(health.0),
                fill: view_mode.fill(),
            })
            .collect();
        Scene {
            background: BACKGROUND,
            grid: Some(Grid {
                spacing: GRID_SPACING,
                line: GRID_LINE,
                axis: GRID_AXIS,
            }),
            quads,
        }
    }
}

pub fn hp_color(hp: i32) -> Color {
    let hp = hp.clamp(0, 100) as u32;
    let red = (255 * (100 - hp) + 50) / 100;
    let green = (255 * hp + 50) / 100;
    [red as u8, green as u8, 0, 255]
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

    fn fresh(&self, seed: u64) -> Self {
        ArenaHost::new(seed)
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

    fn scene(&self, view_mode: ViewMode) -> Scene {
        self.runner.sim().scene(view_mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tickora_render::Fill;

    #[test]
    fn hp_colour_runs_from_red_to_green() {
        assert_eq!(hp_color(0), [255, 0, 0, 255]);
        assert_eq!(hp_color(50), [128, 128, 0, 255]);
        assert_eq!(hp_color(100), [0, 255, 0, 255]);
        assert_eq!(hp_color(-20), hp_color(0));
        assert_eq!(hp_color(400), hp_color(100));
    }

    #[test]
    fn scene_has_one_quad_per_live_entity_in_store_order() {
        let mut host = ArenaHost::new(7);
        host.step(&[json!({"type": "spawn"}), json!({"type": "spawn"})])
            .unwrap();
        let scene = host.scene(ViewMode::Unlit);
        let positions = host.world().query::<Position>();
        assert_eq!(scene.quads.len(), 2);
        for (quad, (_, position)) in scene.quads.iter().zip(positions) {
            assert_eq!(quad.center, [position.x.to_f64(), position.y.to_f64()]);
            assert_eq!(quad.color, hp_color(100));
            assert_eq!(quad.fill, Fill::Solid);
            assert_eq!(quad.half_extent, ENTITY_HALF_EXTENT);
        }
        assert_eq!(scene.background, BACKGROUND);
        assert_eq!(scene.grid.unwrap().spacing, GRID_SPACING);
    }

    #[test]
    fn wireframe_outlines_and_damage_changes_colour() {
        let mut host = ArenaHost::new(7);
        host.step(&[
            json!({"type": "spawn"}),
            json!({"type": "damage", "amount": 50}),
        ])
        .unwrap();
        let quad = host.scene(ViewMode::Wireframe).quads[0];
        assert_eq!(quad.fill, Fill::Outline);
        assert_eq!(quad.color, hp_color(50));
    }

    #[test]
    fn scene_does_not_change_state() {
        let mut host = ArenaHost::new(7);
        host.step(&[json!({"type": "spawn"})]).unwrap();
        let before = host.state_hash();
        host.scene(ViewMode::Unlit);
        assert_eq!(host.state_hash(), before);
    }
}
