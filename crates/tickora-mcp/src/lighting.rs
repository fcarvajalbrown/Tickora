use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde_json::{Value, json};
use tickora_audio::component as audio;
use tickora_audio::{AudioFormat, Block, Capture, MixGraph, Mixer, SoundBank};
use tickora_core::ecs::World;
use tickora_core::fx::Fx;
use tickora_core::hash::StateHasher;
use tickora_core::input::{InputEvent, InputLog};
use tickora_core::noise::Location;
use tickora_core::rng::Rng;
use tickora_core::sim::{Runner, Sim};
use tickora_render::camera::DEFAULT_FOV;
use tickora_render::{
    GoboAnim, GoboRef, Image, Mesh, MeshInstance, PerspectiveCamera, PointLight, Rotator, Scene,
    Scene3, SpotLight, ViewMode,
};

use crate::demo::{BACKGROUND, EXPOSURE, EYE_HEIGHT_CM, WHITE_POINT};
use crate::host::Host;

pub const AMBIENT: [f32; 3] = [0.05, 0.05, 0.05];
pub const LIGHT_INTENSITY: f32 = 4.0;
pub const CEILING_SPOT_Z_CM: f64 = 340.0;
pub const CEILING_SPOT_RADIUS_CM: f64 = 600.0;
pub const POOL_Y_CM: f64 = 220.0;
pub const ROUND_POOL_X_CM: f64 = -350.0;
pub const FLICKER_POOL_X_CM: f64 = 300.0;
pub const TIGHT_SPOT_FALLOFF_POW: f32 = 4.0;
pub const BARS_SPOT: [f64; 3] = [250.0, 0.0, 175.0];
pub const BARS_SPOT_FOV: f64 = 40.0;
pub const BARS_SPOT_ASPECT: f64 = 2.0;
pub const BARS_SPOT_RADIUS_CM: f64 = 600.0;
pub const FLICKER_FRAME_TICKS: u32 = 6;
pub const POINT_LIGHT_Y_CM: f64 = 250.0;
pub const POINT_LIGHT_Z_CM: f64 = 100.0;
pub const POINT_LIGHT_RADIUS_CM: f64 = 300.0;
pub const DOORWAY_X_CM: f64 = -600.0;

const ROOM_GLB: &[u8] = include_bytes!("../../../assets/gen/lighting/meshes/room.glb");
const CRATE_GLB: &[u8] = include_bytes!("../../../assets/gen/lighting/meshes/crate.glb");
const CRATE_GLOSS_DULL_GLB: &[u8] =
    include_bytes!("../../../assets/gen/lighting/meshes/crate_gloss_dull.glb");
const CRATE_GLOSS_SHARP_GLB: &[u8] =
    include_bytes!("../../../assets/gen/lighting/meshes/crate_gloss_sharp.glb");
const LAYOUT_CSV: &str = include_str!("../../../assets/gen/lighting/layout.csv");
const BARS_PNG: &[u8] = include_bytes!("../../../assets/gen/lighting/gobos/bars.png");
const FLICKER_PNGS: [&[u8]; 4] = [
    include_bytes!("../../../assets/gen/lighting/gobos/flicker_0.png"),
    include_bytes!("../../../assets/gen/lighting/gobos/flicker_1.png"),
    include_bytes!("../../../assets/gen/lighting/gobos/flicker_2.png"),
    include_bytes!("../../../assets/gen/lighting/gobos/flicker_3.png"),
];

pub struct Lighting {
    ticks: u64,
    world: World,
}

impl Lighting {
    pub fn new() -> Self {
        let mut world = World::new();
        world.register::<Location>();
        audio::register(&mut world);
        Lighting { ticks: 0, world }
    }

    pub fn world(&self) -> &World {
        &self.world
    }
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting::new()
    }
}

impl Sim for Lighting {
    type Input = ();

    fn step(&mut self, _tick: u64, _inputs: &[InputEvent<()>], _rng: &mut Rng) {
        self.ticks += 1;
    }

    fn hash_state(&self, hasher: &mut StateHasher) {
        hasher.write_u64(self.ticks);
    }
}

pub struct LightingHost {
    seed: u64,
    runner: Runner<Lighting>,
    log: InputLog<()>,
    mixer: Mixer,
    capture: Capture,
}

impl LightingHost {
    pub fn new(seed: u64) -> Self {
        LightingHost {
            seed,
            runner: Runner::new(Lighting::new(), seed),
            log: InputLog::new(),
            mixer: silent_mixer(),
            capture: Capture::new(AudioFormat::DEFAULT),
        }
    }
}

impl Host for LightingHost {
    fn input_schema(&self) -> Value {
        json!({ "not": {} })
    }

    fn fresh(&self, seed: u64) -> Self {
        LightingHost::new(seed)
    }

    fn reset(&mut self, seed: u64) {
        *self = LightingHost::new(seed);
    }

    fn step(&mut self, inputs: &[Value]) -> Result<(), String> {
        if !inputs.is_empty() {
            return Err("the lighting scene takes no inputs".to_string());
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

    fn scene(&self, _view_mode: ViewMode) -> Scene {
        Scene {
            background: BACKGROUND,
            grid: None,
            quads: Vec::new(),
        }
    }

    fn scene3(&self) -> Scene3 {
        scene3(self.runner.tick())
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
        let world = Lighting::new();
        let mut mixer = silent_mixer();
        for tick in 0..to {
            let block = mixer.render(world.world());
            sink(tick, &block, &mixer)?;
        }
        Ok(())
    }
}

fn silent_mixer() -> Mixer {
    let graph = MixGraph::new(Fx::ONE, &[]).expect("an empty mix graph is valid");
    Mixer::new(AudioFormat::DEFAULT, Arc::new(SoundBank::new()), graph)
        .expect("an empty bank feeds an empty graph")
}

pub fn default_camera() -> PerspectiveCamera {
    PerspectiveCamera::new(
        [DOORWAY_X_CM, 0.0, EYE_HEIGHT_CM],
        Rotator::default(),
        DEFAULT_FOV,
    )
    .expect("the lighting camera is valid")
}

pub fn scene3(tick: u64) -> Scene3 {
    let assets = assets();
    Scene3 {
        background: BACKGROUND,
        ambient: AMBIENT,
        exposure: EXPOSURE,
        white_point: WHITE_POINT,
        instances: assets.instances.clone(),
        lights: point_lights(),
        spots: spots(),
        textures: BTreeMap::new(),
        gobos: assets.gobos.clone(),
        materials: BTreeMap::new(),
        tick,
        camera: default_camera(),
    }
}

fn ceiling_spot(x: f64, y: f64) -> SpotLight {
    let mut spot = SpotLight::new([x, y, CEILING_SPOT_Z_CM], Rotator::new(-90.0, 0.0, 0.0));
    spot.intensity = LIGHT_INTENSITY;
    spot.attenuation_radius = CEILING_SPOT_RADIUS_CM;
    spot
}

fn flicker(anim: GoboAnim) -> GoboRef {
    GoboRef {
        name: "flicker".to_string(),
        anim,
        frame_ticks: FLICKER_FRAME_TICKS,
        start_tick: 0,
    }
}

pub fn spots() -> Vec<SpotLight> {
    let round = ceiling_spot(ROUND_POOL_X_CM, POOL_Y_CM);
    let mut tight = ceiling_spot(ROUND_POOL_X_CM, -POOL_Y_CM);
    tight.spot_falloff_pow = TIGHT_SPOT_FALLOFF_POW;
    let mut bars = SpotLight::new(BARS_SPOT, Rotator::default());
    bars.intensity = LIGHT_INTENSITY;
    bars.attenuation_radius = BARS_SPOT_RADIUS_CM;
    bars.fov = BARS_SPOT_FOV;
    bars.aspect = BARS_SPOT_ASPECT;
    bars.gobo = Some(GoboRef::still("bars"));
    let mut looping = ceiling_spot(FLICKER_POOL_X_CM, POOL_Y_CM);
    looping.gobo = Some(flicker(GoboAnim::Loop));
    let mut oscillating = ceiling_spot(FLICKER_POOL_X_CM, -POOL_Y_CM);
    oscillating.gobo = Some(flicker(GoboAnim::Oscillate));
    vec![round, tight, bars, looping, oscillating]
}

pub fn point_lights() -> Vec<PointLight> {
    [(POINT_LIGHT_Y_CM, 2.0), (-POINT_LIGHT_Y_CM, 1.0)]
        .into_iter()
        .map(|(y, falloff_pow)| PointLight {
            location: [0.0, y, POINT_LIGHT_Z_CM],
            color: [1.0, 1.0, 1.0],
            intensity: LIGHT_INTENSITY,
            attenuation_radius: POINT_LIGHT_RADIUS_CM,
            falloff_pow,
            specular_color: None,
            cast_specular: true,
        })
        .collect()
}

struct Assets {
    instances: Vec<MeshInstance>,
    gobos: BTreeMap<String, Vec<Arc<Image>>>,
}

fn assets() -> &'static Assets {
    static ASSETS: OnceLock<Assets> = OnceLock::new();
    ASSETS.get_or_init(|| {
        let mesh = |name: &str, bytes: &[u8]| {
            Arc::new(
                Mesh::from_glb(name, bytes).unwrap_or_else(|e| panic!("embedded {name}.glb: {e}")),
            )
        };
        let meshes = BTreeMap::from([
            ("room", mesh("room", ROOM_GLB)),
            ("crate", mesh("crate", CRATE_GLB)),
            (
                "crate_gloss_dull",
                mesh("crate_gloss_dull", CRATE_GLOSS_DULL_GLB),
            ),
            (
                "crate_gloss_sharp",
                mesh("crate_gloss_sharp", CRATE_GLOSS_SHARP_GLB),
            ),
        ]);
        let instances = layout_rows(LAYOUT_CSV)
            .into_iter()
            .map(|row| {
                let mesh = meshes
                    .get(row.piece.as_str())
                    .unwrap_or_else(|| panic!("layout.csv names an unknown piece {:?}", row.piece));
                let mut instance = MeshInstance::new(mesh.clone(), row.location);
                instance.rotation = Rotator::new(0.0, row.yaw, 0.0);
                instance.scale = row.scale;
                instance
            })
            .collect();
        let png = |name: &str, bytes: &[u8]| {
            Arc::new(Image::from_png(bytes).unwrap_or_else(|e| panic!("embedded {name}.png: {e}")))
        };
        let gobos = BTreeMap::from([
            ("bars".to_string(), vec![png("bars", BARS_PNG)]),
            (
                "flicker".to_string(),
                FLICKER_PNGS
                    .iter()
                    .map(|bytes| png("flicker", bytes))
                    .collect(),
            ),
        ]);
        Assets { instances, gobos }
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutRow {
    pub piece: String,
    pub location: [f64; 3],
    pub yaw: f64,
    pub scale: f64,
}

pub fn layout_rows(csv: &str) -> Vec<LayoutRow> {
    let mut lines = csv.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("layout.csv has no {name} column"))
    };
    let [piece, x, y, z, yaw, scale] =
        ["piece", "x_cm", "y_cm", "z_cm", "yaw_deg", "scale"].map(column);
    lines
        .filter(|line| !line.is_empty())
        .map(|line| {
            let cells: Vec<&str> = line.split(',').collect();
            let number = |index: usize| {
                cells[index]
                    .parse::<f64>()
                    .unwrap_or_else(|e| panic!("layout.csv {line:?}: {e}"))
            };
            LayoutRow {
                piece: cells[piece].to_string(),
                location: [number(x), number(y), number(z)],
                yaw: number(yaw),
                scale: number(scale),
            }
        })
        .filter(|row| !row.piece.starts_with("anchor_"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tickora_render::ShadowQuality;

    #[test]
    fn layout_rows_read_both_rooms_in_centimetres() {
        let rows = layout_rows(LAYOUT_CSV);
        let pieces: Vec<&str> = rows.iter().map(|row| row.piece.as_str()).collect();
        assert_eq!(
            pieces,
            [
                "room",
                "crate",
                "crate",
                "crate",
                "room",
                "crate",
                "crate",
                "crate",
                "crate_gloss_dull",
                "crate_gloss_sharp"
            ]
        );
        assert_eq!(rows[1].location, [-150.0, -150.0, 0.0]);
        assert_eq!(rows[3].location, [400.0, -250.0, 0.0]);
        assert_eq!(rows[4].location, [-2000.0, 0.0, 0.0]);
        assert_eq!(rows[9].location, [-1650.0, 250.0, 0.0]);
        assert!(rows.iter().all(|row| row.yaw == 0.0 && row.scale == 1.0));
    }

    #[test]
    fn layout_rows_skip_anchors_and_blank_lines() {
        let csv = "piece,x_cm,y_cm,z_cm,yaw_deg,scale\nanchor_cam,1,2,3,0,1\n\ncrate,4,5,6,90,2\n";
        assert_eq!(
            layout_rows(csv),
            [LayoutRow {
                piece: "crate".to_string(),
                location: [4.0, 5.0, 6.0],
                yaw: 90.0,
                scale: 2.0,
            }]
        );
    }

    #[test]
    fn the_scene_holds_five_spots_two_points_and_five_gobo_frames() {
        let scene = scene3(0);
        assert_eq!(scene.spots.len(), 5);
        assert_eq!(scene.lights.len(), 2);
        assert_eq!(scene.gobo_layers().unwrap().frames.len(), 5);
        assert!(
            scene
                .light_bytes(&scene.camera, ShadowQuality::default())
                .is_ok()
        );
        assert_eq!(scene.instances.len(), 10);
    }

    #[test]
    fn the_flicker_twins_walk_their_frames_by_tick() {
        let layers = |tick: u64| {
            let scene = scene3(tick);
            [3, 4].map(|index| {
                scene
                    .gobo_layer(scene.spots[index].gobo.as_ref().unwrap())
                    .unwrap()
            })
        };
        assert_eq!(layers(0), [1, 1]);
        assert_eq!(layers(6), [2, 2]);
        assert_eq!(layers(18), [4, 4]);
        assert_eq!(layers(24), [1, 3]);
    }

    #[test]
    fn the_host_takes_no_inputs_and_hashes_only_the_tick() {
        let mut host = LightingHost::new(5);
        assert!(host.step(&[json!({"type": "spawn"})]).is_err());
        let mut twin = LightingHost::new(5);
        for _ in 0..30 {
            host.step(&[]).unwrap();
            twin.step(&[]).unwrap();
        }
        assert_eq!(host.tick(), 30);
        assert_eq!(host.state_hash(), twin.state_hash());
        assert_eq!(host.scene3().tick, 30);
        let mut again = host.fresh(5);
        for _ in 0..30 {
            again.step(&[]).unwrap();
        }
        assert_eq!(again.replay_hash(), host.replay_hash());
        assert_eq!(host.audio().range(0, 30).unwrap().len(), 30);
    }
}
