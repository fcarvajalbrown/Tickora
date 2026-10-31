use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use tickora_render::light::{self, ShadowMap, ShadowSpot};
use tickora_render::math::Vec3;
use tickora_render::{
    AdapterChoice, Display, Image, Material, Mesh, MeshInstance, PerspectiveCamera, PointLight,
    Primitive, Renderer, Rotator, Scene3, ShadowQuality, ShadowResolution, SpotLight, ViewMode,
    tone,
};

const WIDTH: u32 = 200;
const HEIGHT: u32 = 100;
const AMBIENT: f64 = 0.25;
const FLOOR_HALF: f64 = 2000.0;
const PLATE_Z: f64 = 150.0;
const PLATE_HALF: f64 = 40.0;
const SPOT_Z: f64 = 400.0;
const NUDGE_CM: f64 = 1.0e-3;

fn renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(|| {
        let choice = AdapterChoice::from_env().unwrap();
        Renderer::new(choice).unwrap_or_else(|e| panic!("no adapter for render tests: {e}"))
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Plate {
    centre: [f64; 2],
    half: f64,
    z: f64,
}

impl Plate {
    fn contains(&self, x: f64, y: f64) -> bool {
        (x - self.centre[0]).abs() <= self.half && (y - self.centre[1]).abs() <= self.half
    }
}

fn plate_mesh(plate: &Plate, material: &str) -> Arc<Mesh> {
    let [cx, cy] = plate.centre;
    let h = plate.half;
    let z = plate.z as f32;
    let corner = |x: f64, y: f64| [x as f32, y as f32, z];
    Arc::new(Mesh {
        name: "plate".to_string(),
        primitives: vec![Primitive {
            material: material.to_string(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            positions: vec![
                corner(cx - h, cy - h),
                corner(cx + h, cy - h),
                corner(cx + h, cy + h),
                corner(cx - h, cy + h),
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: vec![[0.0, 0.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    })
}

fn floor() -> Plate {
    Plate {
        centre: [0.0, 0.0],
        half: FLOOR_HALF,
        z: 0.0,
    }
}

fn occluder(y: f64) -> Plate {
    Plate {
        centre: [0.0, y],
        half: PLATE_HALF,
        z: PLATE_Z,
    }
}

fn caster(y: f64, resolution: ShadowResolution) -> SpotLight {
    let mut spot = SpotLight::new([0.0, y, SPOT_Z], Rotator::new(-90.0, 0.0, 0.0));
    spot.intensity = 3.0;
    spot.attenuation_radius = 2000.0;
    spot.cast_shadows = true;
    spot.shadow_resolution = resolution;
    spot
}

fn overhead(y: f64, height: f64) -> PerspectiveCamera {
    PerspectiveCamera::new([0.0, y, height], Rotator::new(-90.0, 0.0, 0.0), 90.0).unwrap()
}

fn scene(plates: &[Plate], spots: Vec<SpotLight>, camera: PerspectiveCamera) -> Scene3 {
    Scene3 {
        background: [0, 0, 0, 255],
        ambient: [AMBIENT as f32; 3],
        exposure: 1.0,
        white_point: 11.2,
        instances: plates
            .iter()
            .map(|plate| MeshInstance::new(plate_mesh(plate, "matte"), [0.0; 3]))
            .collect(),
        lights: Vec::new(),
        spots,
        textures: BTreeMap::new(),
        gobos: BTreeMap::new(),
        materials: BTreeMap::new(),
        tick: 0,
        camera,
    }
}

fn render(scene: &Scene3, quality: ShadowQuality) -> Image {
    let display = Display {
        shadow_quality: quality,
        ..Display::default()
    };
    renderer()
        .render3(scene, &scene.camera, ViewMode::Lit, &display, WIDTH, HEIGHT)
        .unwrap()
        .0
}

fn add(a: Vec3, b: Vec3, s: f64) -> Vec3 {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: Vec3) -> Vec3 {
    let length = dot(a, a).sqrt();
    a.map(|v| v / length)
}

fn hit(plates: &[Plate], origin: Vec3, direction: Vec3) -> Option<(usize, f64)> {
    plates
        .iter()
        .enumerate()
        .filter_map(|(index, plate)| {
            let t = (plate.z - origin[2]) / direction[2];
            let point = add(origin, direction, t);
            (t > 0.0 && plate.contains(point[0], point[1])).then_some((index, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn camera_ray(camera: &PerspectiveCamera, x: f64, y: f64) -> Vec3 {
    let [forward, right, up] = camera.rotation.axes();
    let half = (camera.fov.to_radians() / 2.0).tan();
    let aspect = f64::from(WIDTH) / f64::from(HEIGHT);
    let ndc_x = 2.0 * x / f64::from(WIDTH) - 1.0;
    let ndc_y = 1.0 - 2.0 * y / f64::from(HEIGHT);
    add(add(forward, right, ndc_x * half), up, ndc_y * half / aspect)
}

fn shadow_map(spot: &SpotLight, plates: &[Plate]) -> ShadowMap {
    let side = spot.shadow_resolution.side() as usize;
    let [forward, right, up] = spot.rotation.axes();
    let half = (spot.fov.to_radians() / 2.0).tan();
    let bias = spot.shadow_raster_bias();
    let texel_ray = |column: f64, row: f64| {
        let ndc_x = 2.0 * (column + 0.5) / side as f64 - 1.0;
        let ndc_y = 1.0 - 2.0 * (row + 0.5) / side as f64;
        add(
            add(forward, right, ndc_x * half * spot.aspect),
            up,
            ndc_y * half,
        )
    };
    let far = spot.attenuation_radius;
    let plane_depth = |plate: &Plate, direction: Vec3| {
        let distance = (plate.z - spot.location[2]) / direction[2];
        light::shadow_depth(distance, spot.near_plane, far)
    };
    let deepest = |plate: &Plate| {
        let h = plate.half;
        [(-h, -h), (h, -h), (h, h), (-h, h)]
            .into_iter()
            .map(|(dx, dy)| {
                let corner = [plate.centre[0] + dx, plate.centre[1] + dy, plate.z];
                let to_corner = [
                    corner[0] - spot.location[0],
                    corner[1] - spot.location[1],
                    corner[2] - spot.location[2],
                ];
                light::shadow_depth(dot(to_corner, forward), spot.near_plane, far)
            })
            .fold(0.0, f64::max)
    };
    let mut stored = vec![1.0; side * side];
    for row in 0..side {
        for column in 0..side {
            let (x, y) = (column as f64, row as f64);
            let direction = texel_ray(x, y);
            if let Some((index, _)) = hit(plates, spot.location, direction) {
                let plate = &plates[index];
                let depth = plane_depth(plate, direction);
                if depth > 1.0 {
                    continue;
                }
                let slope = (plane_depth(plate, texel_ray(x + 1.0, y)) - depth)
                    .abs()
                    .max((plane_depth(plate, texel_ray(x, y + 1.0)) - depth).abs());
                stored[row * side + column] = bias.apply(depth, deepest(plate), slope);
            }
        }
    }
    ShadowMap { side, stored }
}

struct Reference {
    plates: Vec<Plate>,
    spots: Vec<(SpotLight, ShadowMap)>,
    quality: ShadowQuality,
    camera: PerspectiveCamera,
}

impl Reference {
    fn new(scene: &Scene3, plates: &[Plate], quality: ShadowQuality) -> Self {
        Reference {
            plates: plates.to_vec(),
            spots: scene
                .spots
                .iter()
                .map(|spot| (spot.clone(), shadow_map(spot, plates)))
                .collect(),
            quality,
            camera: scene.camera,
        }
    }

    fn surface(&self, x: f64, y: f64) -> Option<(usize, Vec3)> {
        let direction = camera_ray(&self.camera, x, y);
        hit(&self.plates, self.camera.location, direction)
            .map(|(index, t)| (index, add(self.camera.location, direction, t)))
    }

    fn predict(&self, pixel: [u32; 2]) -> Option<Prediction> {
        let (x, y) = (f64::from(pixel[0]), f64::from(pixel[1]));
        let (surface, point) = self.surface(x + 0.5, y + 0.5)?;
        for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            if self.surface(x + dx, y + dy).map(|s| s.0) != Some(surface) {
                return None;
            }
        }
        let normal = [0.0, 0.0, 1.0];
        let mut linear = AMBIENT;
        let mut penumbra = false;
        let mut swings = Vec::new();
        let mut lowest = 1.0_f64;
        for (spot, map) in &self.spots {
            let to_light = [
                spot.location[0] - point[0],
                spot.location[1] - point[1],
                spot.location[2] - point[2],
            ];
            let distance = dot(to_light, to_light).sqrt();
            let direction = normalize(to_light);
            let forward = spot.rotation.axes()[0];
            let cone = light::spot_cone(
                -dot(direction, forward),
                spot.fov,
                f64::from(spot.spot_falloff_pow),
            );
            let falloff = light::distance_falloff(
                distance,
                spot.attenuation_radius,
                f64::from(spot.falloff_pow),
            );
            let receiver =
                light::shadow_receiver(point, normal, direction, light::SHADOW_NORMAL_OFFSET_CM);
            let lookup = ShadowSpot {
                view_projection: spot.view_projection(),
                location: spot.location,
                forward,
                near_plane: spot.near_plane,
                far_plane: spot.attenuation_radius,
                transition: spot.shadow_transition(),
            };
            let factor = |at: Vec3| light::shadow_factor(map, &lookup, self.quality, at);
            let lambert = dot(normal, direction).max(0.0);
            let reach = f64::from(spot.intensity) * lambert * falloff * cone;
            let shadow = if spot.cast_shadows {
                factor(receiver)
            } else {
                1.0
            };
            if spot.cast_shadows {
                for dx in [-NUDGE_CM, 0.0, NUDGE_CM] {
                    for dy in [-NUDGE_CM, 0.0, NUDGE_CM] {
                        let nudged = [receiver[0] + dx, receiver[1] + dy, receiver[2]];
                        swings.push(reach * (factor(nudged) - shadow));
                    }
                }
            }
            if shadow > 0.0 && shadow < 1.0 && cone > 0.0 && falloff > 0.0 {
                penumbra = true;
            }
            lowest = lowest.min(shadow);
            linear += reach * shadow;
        }
        let boundary = swings
            .iter()
            .any(|swing| byte(linear + swing).abs_diff(byte(linear)) > 1);
        Some(Prediction {
            linear,
            penumbra,
            boundary,
            shadow: lowest,
        })
    }
}

struct Prediction {
    linear: f64,
    penumbra: bool,
    boundary: bool,
    shadow: f64,
}

fn byte(linear: f64) -> u8 {
    tone::encode_byte(
        tone::tone_map(linear, 1.0, tone::DEFAULT_WHITE_POINT),
        tone::DEFAULT_GAMMA,
    )
}

#[derive(Debug, Default)]
struct Tally {
    checked: usize,
    off: Vec<([u32; 2], [u8; 4], u8)>,
    penumbra: usize,
    boundary: usize,
    shadowed: usize,
}

fn check(image: &Image, reference: &Reference) -> Tally {
    let mut tally = Tally::default();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let Some(prediction) = reference.predict([x, y]) else {
                continue;
            };
            if prediction.boundary {
                tally.boundary += 1;
                continue;
            }
            let expected = byte(prediction.linear);
            let actual = image.pixel(x, y);
            tally.checked += 1;
            if expected < byte(AMBIENT + 0.01) {
                tally.shadowed += 1;
            }
            if prediction.penumbra {
                tally.penumbra += 1;
            }
            if !actual[..3].iter().all(|c| c.abs_diff(expected) <= 1) {
                tally.off.push(([x, y], actual, expected));
            }
        }
    }
    tally
}

fn assert_matches(tally: &Tally, label: &str) {
    assert!(
        tally.off.is_empty(),
        "{label}: {} of {} pixels off ({} boundary pixels skipped), first {:?}",
        tally.off.len(),
        tally.checked,
        tally.boundary,
        &tally.off[..tally.off.len().min(5)]
    );
    assert!(
        tally.boundary * 20 <= tally.penumbra,
        "{label}: {} boundary pixels against {} checked penumbra pixels",
        tally.boundary,
        tally.penumbra
    );
    assert!(
        tally.shadowed > 100,
        "{label}: {} shadowed pixels",
        tally.shadowed
    );
}

#[test]
fn a_casting_spot_shadows_the_floor_as_the_reference_predicts() {
    let plates = [floor(), occluder(0.0)];
    let scene = scene(
        &plates,
        vec![caster(0.0, ShadowResolution::High)],
        overhead(-150.0, 300.0),
    );
    for quality in [
        ShadowQuality::Low,
        ShadowQuality::Medium,
        ShadowQuality::High,
    ] {
        let image = render(&scene, quality);
        let tally = check(&image, &Reference::new(&scene, &plates, quality));
        assert_matches(&tally, quality.name());
        if quality != ShadowQuality::Low {
            assert!(tally.penumbra > 0, "{}: no penumbra", quality.name());
        }
    }
}

#[test]
fn low_and_high_maps_beside_each_other_match_their_own_reference() {
    let plates = [floor(), occluder(-300.0), occluder(300.0)];
    let mut low = caster(-300.0, ShadowResolution::Low);
    low.fov = 60.0;
    let mut high = caster(300.0, ShadowResolution::High);
    high.fov = 60.0;
    let scene = scene(&plates, vec![low, high], overhead(0.0, 600.0));
    for quality in [ShadowQuality::Low, ShadowQuality::Medium] {
        let image = render(&scene, quality);
        let tally = check(&image, &Reference::new(&scene, &plates, quality));
        assert_matches(&tally, quality.name());
    }
}

#[test]
fn without_cast_shadows_the_floor_is_the_frame_without_the_occluder() {
    let mut spot = caster(0.0, ShadowResolution::High);
    spot.cast_shadows = false;
    let with_plate = render(
        &scene(
            &[floor(), occluder(0.0)],
            vec![spot.clone()],
            overhead(-150.0, 300.0),
        ),
        ShadowQuality::Medium,
    );
    let without = render(
        &scene(&[floor()], vec![spot], overhead(-150.0, 300.0)),
        ShadowQuality::Medium,
    );
    let reference = Reference::new(
        &scene(
            &[floor(), occluder(0.0)],
            Vec::new(),
            overhead(-150.0, 300.0),
        ),
        &[floor(), occluder(0.0)],
        ShadowQuality::Medium,
    );
    let mut floor_pixels = 0;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let on_floor = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (0.5, 0.5)]
                .iter()
                .all(|(dx, dy)| {
                    reference
                        .surface(f64::from(x) + dx, f64::from(y) + dy)
                        .map(|s| s.0)
                        == Some(0)
                });
            if on_floor {
                floor_pixels += 1;
                assert_eq!(with_plate.pixel(x, y), without.pixel(x, y), "({x}, {y})");
            }
        }
    }
    assert!(floor_pixels > WIDTH as usize * HEIGHT as usize / 2);
}

fn gloss_floor(lamp: PointLight, material: Option<Material>) -> Scene3 {
    let camera =
        PerspectiveCamera::new([-300.0, 0.0, 200.0], Rotator::new(-30.0, 0.0, 0.0), 90.0).unwrap();
    let mut scene = scene(&[floor()], Vec::new(), camera);
    scene.lights = vec![lamp];
    if let Some(material) = material {
        scene.materials.insert("matte".to_string(), material);
    }
    scene
}

fn lamp() -> PointLight {
    PointLight {
        location: [300.0, 0.0, 200.0],
        color: [1.0, 1.0, 1.0],
        intensity: 2.0,
        attenuation_radius: 2000.0,
        falloff_pow: 1.0,
        specular_color: None,
        cast_specular: true,
    }
}

const GLOSS: Material = Material {
    specular: [0.5, 0.5, 0.5],
    specular_power: 0.4,
};

fn floor_point(camera: &PerspectiveCamera, pixel: [u32; 2]) -> Vec3 {
    let direction = camera_ray(camera, f64::from(pixel[0]) + 0.5, f64::from(pixel[1]) + 0.5);
    let t = -camera.location[2] / direction[2];
    add(camera.location, direction, t)
}

fn gloss_prediction(scene: &Scene3, pixel: [u32; 2]) -> [f64; 3] {
    let point = floor_point(&scene.camera, pixel);
    let lamp = scene.lights[0];
    let material = scene.material("matte");
    let normal = [0.0, 0.0, 1.0];
    let to_light = [
        lamp.location[0] - point[0],
        lamp.location[1] - point[1],
        lamp.location[2] - point[2],
    ];
    let distance = dot(to_light, to_light).sqrt();
    let direction = normalize(to_light);
    let to_camera = normalize([
        scene.camera.location[0] - point[0],
        scene.camera.location[1] - point[1],
        scene.camera.location[2] - point[2],
    ]);
    let falloff = lamp.falloff(distance);
    let intensity = f64::from(lamp.intensity);
    let diffuse = AMBIENT + intensity * dot(normal, direction).max(0.0) * falloff;
    let power = light::specular_power(f64::from(material.specular_power));
    let highlight = light::blinn_phong(normal, direction, to_camera, power) * falloff;
    let specular = lamp.specular();
    [0, 1, 2].map(|c| {
        diffuse + f64::from(specular[c]) * intensity * f64::from(material.specular[c]) * highlight
    })
}

fn expect_close(image: &Image, pixel: [u32; 2], linear: [f64; 3]) {
    let actual = image.pixel(pixel[0], pixel[1]);
    for channel in 0..3 {
        let expected = byte(linear[channel]);
        assert!(
            actual[channel].abs_diff(expected) <= 1,
            "pixel {pixel:?} is {actual:?}, channel {channel} expected {expected}"
        );
    }
}

fn mirror_pixel(scene: &Scene3) -> [u32; 2] {
    let mut best = ([0, 0], f64::MIN);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let value = gloss_prediction(scene, [x, y])[0];
            if value > best.1 {
                best = ([x, y], value);
            }
        }
    }
    best.0
}

#[test]
fn a_point_light_highlight_matches_the_reference_at_the_mirror_angle() {
    let scene = gloss_floor(lamp(), Some(GLOSS));
    let image = render(&scene, ShadowQuality::Medium);
    let mirror = mirror_pixel(&scene);
    let matte = render(&gloss_floor(lamp(), None), ShadowQuality::Medium);
    let gain = |image: &Image| {
        let p = image.pixel(mirror[0], mirror[1]);
        u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])
    };
    assert!(
        gain(&image) > gain(&matte) + 30,
        "{} vs {}",
        gain(&image),
        gain(&matte)
    );
    for pixel in [
        mirror,
        [mirror[0] + 10, mirror[1]],
        [mirror[0] - 10, mirror[1] + 5],
        [mirror[0], mirror[1] - 8],
    ] {
        expect_close(&image, pixel, gloss_prediction(&scene, pixel));
    }
}

#[test]
fn no_material_or_no_cast_specular_leaves_the_frame_as_it_was() {
    let matte = render(&gloss_floor(lamp(), None), ShadowQuality::Medium);
    let mut off = lamp();
    off.cast_specular = false;
    let unlit_gloss = render(&gloss_floor(off, Some(GLOSS)), ShadowQuality::Medium);
    let dull = render(
        &gloss_floor(lamp(), Some(Material::default())),
        ShadowQuality::Medium,
    );
    assert_eq!(matte.pixel_hash(), unlit_gloss.pixel_hash());
    assert_eq!(matte.pixel_hash(), dull.pixel_hash());
}

#[test]
fn a_specular_colour_tints_the_highlight() {
    let mut tinted = lamp();
    tinted.specular_color = Some([1.0, 0.0, 0.0]);
    let scene = gloss_floor(tinted, Some(GLOSS));
    let image = render(&scene, ShadowQuality::Medium);
    let mirror = mirror_pixel(&scene);
    let actual = image.pixel(mirror[0], mirror[1]);
    assert!(actual[0] > actual[1] + 10, "{actual:?}");
    expect_close(&image, mirror, gloss_prediction(&scene, mirror));
}

#[test]
fn bad_shadow_and_specular_inputs_are_errors() {
    let fails = |scene: &Scene3| {
        renderer()
            .render3(
                scene,
                &scene.camera,
                ViewMode::Lit,
                &Display::default(),
                WIDTH,
                HEIGHT,
            )
            .is_err()
    };
    let plates = [floor()];
    let casters: Vec<SpotLight> = (0..5)
        .map(|i| caster(f64::from(i) * 100.0, ShadowResolution::Low))
        .collect();
    assert!(fails(&scene(
        &plates,
        casters.clone(),
        overhead(-150.0, 300.0)
    )));
    assert!(!fails(&scene(
        &plates,
        casters[..4].to_vec(),
        overhead(-150.0, 300.0)
    )));
    let spoilers: [fn(&mut SpotLight); 4] = [
        |s| s.shadow_bias_mul = -1.0,
        |s| s.shadow_slope_bias_mul = f32::NAN,
        |s| s.specular_color = Some([f32::NAN, 0.0, 0.0]),
        |s| s.specular_color = Some([-1.0, 0.0, 0.0]),
    ];
    for (index, spoil) in spoilers.iter().enumerate() {
        let mut spot = caster(0.0, ShadowResolution::High);
        spoil(&mut spot);
        assert!(
            fails(&scene(&plates, vec![spot], overhead(-150.0, 300.0))),
            "case {index}"
        );
    }
    for bad in [
        Material {
            specular: [-1.0, 0.0, 0.0],
            specular_power: 0.5,
        },
        Material {
            specular: [f32::INFINITY, 0.0, 0.0],
            specular_power: 0.5,
        },
        Material {
            specular: [1.0; 3],
            specular_power: 1.01,
        },
        Material {
            specular: [1.0; 3],
            specular_power: -0.01,
        },
    ] {
        assert!(fails(&gloss_floor(lamp(), Some(bad))), "{bad:?}");
    }
}

#[test]
fn shadows_and_materials_do_nothing_unlit_or_in_wireframe() {
    let plates = [floor(), occluder(0.0)];
    let mut casting = scene(
        &plates,
        vec![caster(0.0, ShadowResolution::High)],
        overhead(-150.0, 300.0),
    );
    let mut plain = casting.clone();
    plain.spots[0].cast_shadows = false;
    casting.materials.insert("matte".to_string(), GLOSS);
    for mode in [ViewMode::Unlit, ViewMode::Wireframe] {
        let frame = |scene: &Scene3| {
            renderer()
                .render3(
                    scene,
                    &scene.camera,
                    mode,
                    &Display::default(),
                    WIDTH,
                    HEIGHT,
                )
                .unwrap()
                .0
                .pixel_hash()
        };
        assert_eq!(frame(&casting), frame(&plain), "{mode:?}");
    }
}

fn geometrically_shadowed(plates: &[Plate], point: Vec3, light: Vec3) -> bool {
    let direction = [
        light[0] - point[0],
        light[1] - point[1],
        light[2] - point[2],
    ];
    plates.iter().any(|plate| {
        let t = (plate.z - point[2]) / direction[2];
        let at = add(point, direction, t);
        t > 1.0e-6 && t < 1.0 && plate.contains(at[0], at[1])
    })
}

#[test]
fn off_axis_occluders_shadow_the_side_away_from_their_lights() {
    let mut low_plate = occluder(-300.0);
    low_plate.centre[0] = 90.0;
    let mut high_plate = occluder(300.0);
    high_plate.centre[0] = -90.0;
    let plates = [floor(), low_plate, high_plate];
    let mut low = caster(-300.0, ShadowResolution::Low);
    low.fov = 60.0;
    let mut high = caster(300.0, ShadowResolution::High);
    high.fov = 60.0;
    let scene = scene(&plates, vec![low, high], overhead(0.0, 600.0));
    let reference = Reference::new(&scene, &plates, ShadowQuality::Low);
    let image = render(&scene, ShadowQuality::Low);
    let lights: Vec<Vec3> = scene.spots.iter().map(|spot| spot.location).collect();
    let (mut agree, mut disagree, mut shadowed) = (0, 0, 0);
    let mut off = Vec::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let Some(prediction) = reference.predict([x, y]) else {
                continue;
            };
            let Some((0, point)) = reference.surface(f64::from(x) + 0.5, f64::from(y) + 0.5) else {
                continue;
            };
            if prediction.boundary {
                continue;
            }
            let lit_in_reference = prediction.shadow > 0.5;
            let shadow = lights.iter().any(|light| {
                let reach = (point[1] - light[1]).abs() < 250.0;
                reach && geometrically_shadowed(&plates[1..], point, *light)
            });
            shadowed += usize::from(shadow);
            if lit_in_reference != shadow {
                agree += 1;
            } else {
                disagree += 1;
            }
            let expected = byte(prediction.linear);
            let actual = image.pixel(x, y);
            if !actual[..3].iter().all(|c| c.abs_diff(expected) <= 1) {
                off.push(([x, y], actual[0], expected, point, shadow));
            }
        }
    }
    assert!(
        off.is_empty(),
        "{} pixels off the reference, first {:?}",
        off.len(),
        &off[..off.len().min(8)]
    );
    assert!(shadowed > 50, "{shadowed} geometrically shadowed pixels");
    assert!(
        disagree * 100 <= agree,
        "reference and geometry disagree on {disagree} of {} floor pixels",
        agree + disagree
    );
}
