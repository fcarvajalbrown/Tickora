use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};
use tickora_audio::component::{self as audio, audio_rng};
use tickora_audio::{
    Attenuation, AudioFormat, Block, Capture, Envelope, Listener, MixGraph, Mixer, Pick, SoundBank,
    SoundDef, Source, SubmixDef, TakeHistory, Variation, wav,
};
use tickora_core::ecs::{Component, Entity, FieldValue, Fields, World};
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::input::{InputEvent, InputLog};
use tickora_core::noise::Location;
use tickora_core::rng::Rng;
use tickora_core::sim::{Runner, Sim};
use tickora_render::camera::DEFAULT_FOV;
use tickora_render::tone;
use tickora_render::{
    Color, Grid, Image, Mesh, MeshInstance, PerspectiveCamera, PointLight, Quad, Rotator, Scene,
    Scene3, ViewMode,
};

use crate::host::Host;

pub const BACKGROUND: Color = [24, 24, 28, 255];
pub const GRID_LINE: Color = [48, 48, 56, 255];
pub const GRID_AXIS: Color = [96, 96, 110, 255];
pub const GRID_SPACING: f64 = 32.0;
pub const ENTITY_HALF_EXTENT: f64 = 2.0;
pub const SPAWN_SPAN: u32 = 100;
pub const UNIT_CM: f64 = 25.0;
pub const EYE_HEIGHT_CM: f64 = 170.0;
pub const AMBIENT: [f32; 3] = [0.15, 0.15, 0.15];
pub const EXPOSURE: f32 = 1.0;
pub const WHITE_POINT: f32 = 11.2;
pub const LIGHT_HEIGHT_CM: f64 = 300.0;
pub const LIGHT_INTENSITY: f32 = 30.0;
pub const LIGHT_RADIUS_CM: f64 = 2000.0;
pub const SFX_GAIN: Fx = Fx::from_raw(1 << 31);
pub const SPAWN_SOUND: &str = "spawn";
pub const HIT_SOUND: &str = "hit";

const CUBE_GLB: &[u8] = include_bytes!("../../../assets/gen/render_fixtures/meshes/cube.glb");
const FLOOR_GLB: &[u8] = include_bytes!("../../../assets/gen/render_fixtures/meshes/floor.glb");
const CHECKER_PNG: &[u8] =
    include_bytes!("../../../assets/gen/render_fixtures/textures/checker.png");
const SPAWN_WAV: &[u8] = include_bytes!("../../../assets/gen/audio/spawn.wav");
const HIT_WAVS: [&[u8]; 3] = [
    include_bytes!("../../../assets/gen/audio/hit_01.wav"),
    include_bytes!("../../../assets/gen/audio/hit_02.wav"),
    include_bytes!("../../../assets/gen/audio/hit_03.wav"),
];

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
    audio_rng: Rng,
    takes: TakeHistory,
}

impl Arena {
    pub fn new(seed: u64) -> Self {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Health>();
        world.register::<Location>();
        audio::register(&mut world);
        let listener = world.spawn();
        world
            .insert(listener, default_listener())
            .expect("audio::register was called on this world");
        Arena {
            world,
            audio_rng: audio_rng(seed),
            takes: TakeHistory::new(),
        }
    }

    fn play(&mut self, sound: &str, at: &Position) {
        let bank = sound_bank();
        let sound = bank.id(sound).expect("the arena bank has its sounds");
        audio::play(
            &mut self.world,
            bank,
            &mut self.audio_rng,
            &mut self.takes,
            sound,
            sound_location(at),
        )
        .expect("audio::register was called on this world");
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

    pub fn scene3(&self) -> Scene3 {
        let assets = assets();
        let mut floor = MeshInstance::new(assets.floor.clone(), [0.0; 3]);
        floor.scale = spawn_span_cm() / assets.floor_width_cm;
        let mut instances = vec![floor];
        instances.extend(self.world.query2::<Position, Health>().into_iter().map(
            |(_, position, health)| {
                let mut cube = MeshInstance::new(assets.cube.clone(), unreal_location(position));
                cube.color = Some(hp_color_linear(health.0));
                cube
            },
        ));
        let quarter = spawn_span_cm() / 4.0;
        let lights = [-quarter, quarter]
            .into_iter()
            .map(|y| PointLight {
                location: [-quarter, y, LIGHT_HEIGHT_CM],
                color: [1.0, 1.0, 1.0],
                intensity: LIGHT_INTENSITY,
                attenuation_radius: LIGHT_RADIUS_CM,
            })
            .collect();
        let textures = assets
            .floor
            .primitives
            .iter()
            .map(|primitive| (primitive.material.clone(), assets.checker.clone()))
            .collect::<BTreeMap<_, _>>();
        Scene3 {
            background: BACKGROUND,
            ambient: AMBIENT,
            exposure: EXPOSURE,
            white_point: WHITE_POINT,
            instances,
            lights,
            textures,
            camera: default_camera(),
        }
    }
}

struct Assets {
    cube: Arc<Mesh>,
    floor: Arc<Mesh>,
    floor_width_cm: f64,
    checker: Arc<Image>,
}

fn assets() -> &'static Assets {
    static ASSETS: OnceLock<Assets> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let mesh = |name: &str, bytes: &[u8]| {
            Mesh::from_glb(name, bytes).unwrap_or_else(|e| panic!("embedded {name}.glb: {e}"))
        };
        let floor = mesh("floor", FLOOR_GLB);
        let (min, max) = floor.bounds().expect("embedded floor.glb has vertices");
        Assets {
            cube: Arc::new(mesh("cube", CUBE_GLB)),
            floor_width_cm: f64::from(max[0] - min[0]),
            floor: Arc::new(floor),
            checker: Arc::new(
                Image::from_png(CHECKER_PNG)
                    .unwrap_or_else(|e| panic!("embedded checker.png: {e}")),
            ),
        }
    })
}

fn spawn_span_cm() -> f64 {
    f64::from(SPAWN_SPAN) * UNIT_CM
}

pub fn unreal_location(position: &Position) -> [f64; 3] {
    [
        position.y.to_f64() * UNIT_CM,
        position.x.to_f64() * UNIT_CM,
        0.0,
    ]
}

pub fn sound_location(position: &Position) -> Location {
    let unit = Fx::from_int(UNIT_CM as i32);
    Location::new(position.y * unit, position.x * unit, Fx::ZERO)
}

pub fn default_listener() -> Listener {
    let camera = default_camera();
    let [x, y, z] = camera.location.map(|cm| Fx::from_int(cm as i32));
    Listener {
        location: Location::new(x, y, z),
        yaw: Fx::from_int(camera.rotation.yaw as i32),
    }
}

pub fn mix_graph() -> MixGraph {
    MixGraph::new(
        Fx::ONE,
        &[SubmixDef {
            name: "sfx".to_string(),
            parent: "master".to_string(),
            gain: SFX_GAIN,
        }],
    )
    .expect("the arena mix graph is valid")
}

pub fn sound_bank() -> &'static Arc<SoundBank> {
    static BANK: OnceLock<Arc<SoundBank>> = OnceLock::new();
    BANK.get_or_init(|| {
        let sfx = mix_graph().id("sfx").expect("the arena graph has sfx");
        let attenuation = Attenuation::Linear {
            inner_radius: Fx::from_int(500),
            falloff_distance: Fx::from_int(5000),
        };
        let mut bank = SoundBank::new();
        bank.add(
            SPAWN_SOUND,
            SoundDef {
                source: Source::Pcm {
                    samples: baked(SPAWN_WAV),
                    rate: wav::READ_SAMPLE_RATE,
                },
                envelope: Envelope::default(),
                variation: Variation::default(),
                attenuation,
                submix: sfx,
            },
        )
        .expect("spawn is a valid sound");
        bank.add(
            HIT_SOUND,
            SoundDef {
                source: Source::Takes {
                    takes: HIT_WAVS.map(baked).into(),
                    rate: wav::READ_SAMPLE_RATE,
                    weights: Arc::from([]),
                    pick: Pick::NoRepeats(1),
                },
                envelope: Envelope::default(),
                variation: Variation {
                    gain: (Fx::from_ratio(1, 2), Fx::from_ratio(3, 4)),
                    pitch: (Fx::from_ratio(4, 5), Fx::from_ratio(6, 5)),
                },
                attenuation,
                submix: sfx,
            },
        )
        .expect("hit is a valid sound");
        Arc::new(bank)
    })
}

fn baked(bytes: &[u8]) -> Arc<[i32]> {
    wav::read_mono16(bytes)
        .expect("assets/gen/audio holds 48 kHz mono 16-bit WAVs")
        .into()
}

pub fn arena_mixer() -> Mixer {
    Mixer::new(AudioFormat::DEFAULT, sound_bank().clone(), mix_graph())
        .expect("the arena bank feeds the arena graph")
}

pub fn default_camera() -> PerspectiveCamera {
    PerspectiveCamera::new(
        [-spawn_span_cm(), 0.0, EYE_HEIGHT_CM],
        Rotator::default(),
        DEFAULT_FOV,
    )
    .expect("the arena camera is valid")
}

pub fn hp_color_linear(hp: i32) -> [f32; 4] {
    let [red, green, blue, _] = hp_color(hp).map(|channel| tone::decode_byte(channel) as f32);
    [red, green, blue, 1.0]
}

pub fn hp_color(hp: i32) -> Color {
    let hp = hp.clamp(0, 100) as u32;
    let red = (255 * (100 - hp) + 50) / 100;
    let green = (255 * hp + 50) / 100;
    [red as u8, green as u8, 0, 255]
}

impl Sim for Arena {
    type Input = Command;

    fn step(&mut self, _tick: u64, inputs: &[InputEvent<Command>], rng: &mut Rng) {
        audio::clear(&mut self.world);
        for event in inputs {
            match event.input {
                Command::Spawn => {
                    let e = self.world.spawn();
                    let spawn = |rng: &mut Rng| {
                        Fx::from_int(rng.below(SPAWN_SPAN) as i32 - (SPAWN_SPAN / 2) as i32)
                    };
                    let (px, py) = (spawn(rng), spawn(rng));
                    let (vx, vy) = (spawn(rng) / Fx::from_int(10), spawn(rng) / Fx::from_int(10));
                    self.world.insert(e, Position { x: px, y: py }).unwrap();
                    self.world.insert(e, Velocity { x: vx, y: vy }).unwrap();
                    self.world.insert(e, Health(100)).unwrap();
                    self.play(SPAWN_SOUND, &Position { x: px, y: py });
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
                    let at = self.world.get::<Position>(target).unwrap().clone();
                    self.play(HIT_SOUND, &at);
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
        let (state, increment) = self.audio_rng.raw_parts();
        hasher.write_u64(state);
        hasher.write_u64(increment);
        self.takes.hash_into(hasher);
    }
}

pub struct ArenaHost {
    seed: u64,
    runner: Runner<Arena>,
    log: InputLog<Command>,
    mixer: Mixer,
    capture: Capture,
}

impl ArenaHost {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            runner: Runner::new(Arena::new(seed), seed),
            log: InputLog::new(),
            mixer: arena_mixer(),
            capture: Capture::new(AudioFormat::DEFAULT),
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
        let block = self.mixer.render(self.runner.sim().world());
        self.capture.push(block.summary);
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

    fn scene3(&self) -> Scene3 {
        self.runner.sim().scene3()
    }

    fn audio(&self) -> &Capture {
        &self.capture
    }

    fn replay_audio(
        &self,
        to: u64,
        sink: &mut dyn FnMut(u64, &Block, &Mixer) -> Result<(), String>,
    ) -> Result<(), String> {
        self.capture.range(0, to)?;
        let mut runner = Runner::new(Arena::new(self.seed), self.seed);
        let mut mixer = arena_mixer();
        for tick in 0..to {
            runner.step(&self.log);
            let block = mixer.render(runner.sim().world());
            sink(tick, &block, &mixer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tickora_audio::component as audio_types;
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
        host.scene3();
        assert_eq!(host.state_hash(), before);
    }

    #[test]
    fn scene3_has_the_floor_then_one_metre_cube_per_entity_coloured_by_hp() {
        let mut host = ArenaHost::new(7);
        host.step(&[
            json!({"type": "spawn"}),
            json!({"type": "spawn"}),
            json!({"type": "damage", "amount": 50}),
        ])
        .unwrap();
        let scene = host.scene3();
        let entities = host.world().query2::<Position, Health>();
        assert_eq!(scene.instances.len(), 1 + entities.len());
        let floor = &scene.instances[0];
        let (floor_min, floor_max) = floor.mesh.bounds().unwrap();
        let floor_scale = floor.scale as f32;
        assert_eq!((floor_max[0] - floor_min[0]) * floor_scale, 2500.0);
        assert_eq!(floor_max[2] * floor_scale, 0.0);
        for (cube, (_, position, health)) in scene.instances[1..].iter().zip(entities) {
            assert_eq!(
                cube.mesh.bounds().unwrap(),
                ([-50.0, -50.0, 0.0], [50.0, 50.0, 100.0])
            );
            assert_eq!(cube.location, unreal_location(position));
            assert_eq!(cube.color, Some(hp_color_linear(health.0)));
        }
        assert_eq!(scene.lights.len(), 2);
        assert!(scene.textures.contains_key("dirt"));
        assert_eq!(scene.camera, default_camera());
        assert_eq!(scene.camera.location, [-2500.0, 0.0, EYE_HEIGHT_CM]);
    }

    #[test]
    fn spawns_and_hits_play_sounds_at_the_entity_for_one_tick() {
        let mut host = ArenaHost::new(7);
        assert_eq!(host.world().query::<Listener>().len(), 1);
        host.step(&[json!({"type": "spawn"})]).unwrap();
        let plays = host.world().query::<audio_types::PlaySound>();
        let (entity, moved) = host.world().query::<Position>()[0];
        let velocity = host.world().get::<Velocity>(entity).unwrap();
        let cube = Position {
            x: moved.x - velocity.x,
            y: moved.y - velocity.y,
        };
        assert_eq!(plays.len(), 1);
        assert_eq!(plays[0].1.sound, sound_bank().id(SPAWN_SOUND).unwrap());
        assert_eq!(plays[0].1.location, sound_location(&cube));
        host.step(&[json!({"type": "damage", "amount": 1})])
            .unwrap();
        let plays = host.world().query::<audio_types::PlaySound>();
        assert_eq!(plays.len(), 1);
        assert_eq!(plays[0].1.sound, sound_bank().id(HIT_SOUND).unwrap());
        host.step(&[]).unwrap();
        assert!(host.world().query::<audio_types::PlaySound>().is_empty());
        let meters = host.audio().meters(0, 3).unwrap();
        assert!(meters.peak_dbfs[0].is_some() && meters.peak_dbfs[1].is_some());
        assert_eq!(meters.clipped, 0);
        assert_eq!(host.audio().meters(0, 1).unwrap().voices, 1);
    }

    #[test]
    fn the_replay_names_each_sound_at_the_tick_its_input_set() {
        let mut host = ArenaHost::new(7);
        host.step(&[json!({"type": "spawn"})]).unwrap();
        host.step(&[json!({"type": "damage", "amount": 1})])
            .unwrap();
        host.step(&[]).unwrap();
        let mut starts = Vec::new();
        host.replay_audio(3, &mut |tick, block, mixer| {
            for event in &block.events {
                if let tickora_audio::VoiceEvent::Start(start) = event {
                    let name = mixer.bank().name(start.sound).unwrap().to_string();
                    let submix = mixer.graph().name(start.submix).unwrap().to_string();
                    starts.push((tick, name, submix));
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            starts,
            vec![
                (0, SPAWN_SOUND.to_string(), "sfx".to_string()),
                (1, HIT_SOUND.to_string(), "sfx".to_string()),
            ]
        );
    }

    #[test]
    fn a_wav_replays_the_input_log_to_the_same_audio() {
        let mut host = ArenaHost::new(7);
        host.step(&[json!({"type": "spawn"})]).unwrap();
        host.step(&[json!({"type": "damage", "amount": 1})])
            .unwrap();
        host.step(&[]).unwrap();
        let mut bytes = Vec::new();
        crate::host::write_wav(&host, 1, 3, &mut bytes).unwrap();
        assert_eq!(bytes.len(), wav::HEADER_BYTES + 2 * 800 * 6);
        let frames: Vec<[i32; 2]> = bytes[wav::HEADER_BYTES..]
            .chunks(6)
            .map(|frame| {
                let sample = |b: &[u8]| i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
                [sample(&frame[..3]), sample(&frame[3..])]
            })
            .collect();
        let mut hasher = StateHasher::new();
        for block in frames.chunks(800) {
            let mut block_hasher = StateHasher::new();
            for value in block.iter().flatten() {
                block_hasher.write_bytes(&value.to_le_bytes()[..3]);
            }
            hasher.write_u64(block_hasher.finish());
        }
        assert_eq!(hasher.finish(), host.audio().hash(1, 3).unwrap());
        assert!(crate::host::write_wav(&host, 0, 4, &mut Vec::new()).is_err());
    }

    #[test]
    fn top_view_matches_a_camera_looking_straight_down() {
        let position = Position {
            x: Fx::from_int(2),
            y: Fx::from_int(-3),
        };
        assert_eq!(unreal_location(&position), [-75.0, 50.0, 0.0]);
        let [_, right, up] = Rotator::new(-90.0, 0.0, 0.0).axes();
        let close = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-12);
        assert!(close(right, [0.0, 1.0, 0.0]));
        assert!(close(up, [1.0, 0.0, 0.0]));
    }
}
