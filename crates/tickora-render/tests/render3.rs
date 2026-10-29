use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use tickora_render::{
    AdapterChoice, Display, GoboAnim, GoboRef, Image, Mesh, MeshInstance, PerspectiveCamera,
    PointLight, Renderer, Rotator, Scene3, SpotLight, ViewMode, light, tone,
};

const BACKGROUND: [u8; 4] = [24, 24, 28, 255];
const STONE: [u8; 4] = [120, 118, 112, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const WIDTH: u32 = 200;
const HEIGHT: u32 = 100;

fn renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(|| {
        let choice = AdapterChoice::from_env().unwrap();
        Renderer::new(choice).unwrap_or_else(|e| panic!("no adapter for render tests: {e}"))
    })
}

fn cube() -> Arc<Mesh> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/gen/render_fixtures/meshes/cube.glb");
    Arc::new(Mesh::load(&path).unwrap())
}

fn eye() -> PerspectiveCamera {
    PerspectiveCamera::new([0.0, 0.0, 50.0], Rotator::default(), 90.0).unwrap()
}

fn near_and_far(near_first: bool) -> Scene3 {
    let cube = cube();
    let near = MeshInstance::new(cube.clone(), [300.0, 0.0, 0.0]);
    let mut far = MeshInstance::new(cube, [600.0, 0.0, 0.0]);
    far.scale = 3.0;
    far.color = Some([1.0, 0.0, 0.0, 1.0]);
    let instances = if near_first {
        vec![near, far]
    } else {
        vec![far, near]
    };
    Scene3 {
        background: BACKGROUND,
        ambient: [0.0; 3],
        exposure: 1.0,
        white_point: 11.2,
        instances,
        lights: Vec::new(),
        spots: Vec::new(),
        textures: BTreeMap::new(),
        gobos: BTreeMap::new(),
        tick: 0,
        camera: eye(),
    }
}

fn pixel_at(image: &Image, ndc_x: f64, ndc_y: f64) -> [u8; 4] {
    let x = ((ndc_x + 1.0) / 2.0 * f64::from(image.width)) as u32;
    let y = ((1.0 - ndc_y) / 2.0 * f64::from(image.height)) as u32;
    image.pixel(x, y)
}

fn render(scene: &Scene3, view_mode: ViewMode) -> (Image, usize) {
    renderer()
        .render3(
            scene,
            &scene.camera,
            view_mode,
            &Display::default(),
            WIDTH,
            HEIGHT,
        )
        .unwrap()
}

#[test]
fn near_cube_hides_the_far_one() {
    let (image, off_screen) = render(&near_and_far(true), ViewMode::Unlit);
    assert_eq!(off_screen, 0);
    assert_eq!(pixel_at(&image, 0.0, 0.05), STONE);
    assert_eq!(pixel_at(&image, 0.27, 0.05), RED);
    assert_eq!(pixel_at(&image, 0.8, 0.05), BACKGROUND);
    assert_eq!(pixel_at(&image, 0.0, -0.9), BACKGROUND);
}

#[test]
fn draw_order_does_not_change_the_frame() {
    let (first, _) = render(&near_and_far(true), ViewMode::Unlit);
    let (second, _) = render(&near_and_far(false), ViewMode::Unlit);
    assert_eq!(first.pixel_hash(), second.pixel_hash());
}

#[test]
fn wireframe_draws_edges_and_leaves_faces_open() {
    let (unlit, _) = render(&near_and_far(true), ViewMode::Unlit);
    let (wireframe, _) = render(&near_and_far(true), ViewMode::Wireframe);
    let count = |image: &Image, color: [u8; 4]| {
        (0..image.height)
            .flat_map(|y| (0..image.width).map(move |x| (x, y)))
            .filter(|&(x, y)| image.pixel(x, y) == color)
            .count()
    };
    let lines = count(&wireframe, STONE);
    assert!(lines > 0, "no cube edges drawn");
    assert!(lines < count(&unlit, STONE) / 2, "{lines} edge pixels");
    assert_eq!(pixel_at(&wireframe, 0.1, -0.05), BACKGROUND);
}

#[test]
fn counts_instances_behind_the_camera() {
    let mut scene = near_and_far(true);
    scene
        .instances
        .push(MeshInstance::new(cube(), [-500.0, 0.0, 0.0]));
    let (_, off_screen) = render(&scene, ViewMode::Unlit);
    assert_eq!(off_screen, 1);
}

#[test]
fn rejects_sizes_out_of_range() {
    let scene = near_and_far(true);
    let renderer = renderer();
    assert!(
        renderer
            .render3(
                &scene,
                &scene.camera,
                ViewMode::Unlit,
                &Display::default(),
                15,
                64
            )
            .is_err()
    );
}

#[test]
fn plus_y_is_screen_right_and_plus_z_is_screen_up() {
    let scene = |location: [f64; 3]| Scene3 {
        background: BACKGROUND,
        ambient: [0.0; 3],
        exposure: 1.0,
        white_point: 11.2,
        instances: vec![MeshInstance::new(cube(), location)],
        lights: Vec::new(),
        spots: Vec::new(),
        textures: BTreeMap::new(),
        gobos: BTreeMap::new(),
        tick: 0,
        camera: eye(),
    };
    let (right, _) = render(&scene([300.0, 200.0, 0.0]), ViewMode::Unlit);
    assert_eq!(pixel_at(&right, 0.8, 0.0), STONE);
    assert_eq!(pixel_at(&right, -0.8, 0.0), BACKGROUND);
    let (up, _) = render(&scene([300.0, 0.0, 150.0]), ViewMode::Unlit);
    assert_eq!(pixel_at(&up, 0.0, 0.9), STONE);
    assert_eq!(pixel_at(&up, 0.0, -0.9), BACKGROUND);
}

#[test]
fn psx_bake_texture_shows_its_texels_unlit() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/gen/render_fixtures/textures/checker.png");
    let checker = Image::from_png(&std::fs::read(path).unwrap()).unwrap();
    let mut cube = MeshInstance::new(cube(), [300.0, 0.0, 0.0]);
    cube.color = Some([1.0, 1.0, 1.0, 1.0]);
    let scene = Scene3 {
        background: BACKGROUND,
        ambient: [0.0; 3],
        exposure: 1.0,
        white_point: 11.2,
        instances: vec![cube],
        lights: Vec::new(),
        spots: Vec::new(),
        textures: BTreeMap::from([("stone".to_string(), Arc::new(checker))]),
        gobos: BTreeMap::new(),
        tick: 0,
        camera: eye(),
    };
    let (image, _) = render(&scene, ViewMode::Unlit);
    assert_eq!(pixel_at(&image, -0.1, 0.05), [200, 40, 40, 255]);
    assert_eq!(pixel_at(&image, 0.1, 0.05), [40, 40, 200, 255]);
    let (wireframe, _) = render(&scene, ViewMode::Wireframe);
    assert_eq!(pixel_at(&wireframe, 0.1, -0.05), BACKGROUND);
}

fn lit_cube(lights: Vec<PointLight>) -> Scene3 {
    let mut cube = MeshInstance::new(cube(), [300.0, 0.0, 0.0]);
    cube.color = Some([1.0, 1.0, 1.0, 1.0]);
    Scene3 {
        background: BACKGROUND,
        ambient: [0.25, 0.25, 0.25],
        exposure: 1.0,
        white_point: 11.2,
        instances: vec![cube],
        lights,
        spots: Vec::new(),
        textures: BTreeMap::new(),
        gobos: BTreeMap::new(),
        tick: 0,
        camera: eye(),
    }
}

fn lamp(x: f64) -> PointLight {
    PointLight {
        location: [x, 0.0, 50.0],
        color: [1.0, 1.0, 1.0],
        intensity: 1.0,
        attenuation_radius: 1000.0,
        falloff_pow: 1.0,
    }
}

fn luma(pixel: [u8; 4]) -> u32 {
    u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2])
}

fn within_one(pixel: [u8; 4], expected: u8) -> bool {
    pixel[..3]
        .iter()
        .all(|&channel| channel.abs_diff(expected) <= 1)
        && pixel[3] == 255
}

fn tone_mapped(linear: f64, exposure: f64) -> u8 {
    tone::encode_byte(
        tone::tone_map(linear, exposure, tone::DEFAULT_WHITE_POINT),
        tone::DEFAULT_GAMMA,
    )
}

#[test]
fn lit_ambient_alone_follows_the_tone_curve() {
    let (image, _) = render(&lit_cube(Vec::new()), ViewMode::Lit);
    let centre = pixel_at(&image, 0.0, 0.05);
    assert_eq!(tone_mapped(0.25, 1.0), 86);
    assert!(within_one(centre, 86), "{centre:?}");
}

#[test]
fn exposure_brightens_a_lit_face() {
    let mut scene = lit_cube(Vec::new());
    scene.exposure = 2.0;
    let (image, _) = render(&scene, ViewMode::Lit);
    let centre = pixel_at(&image, 0.0, 0.05);
    assert!(within_one(centre, tone_mapped(0.25, 2.0)), "{centre:?}");
}

#[test]
fn light_past_the_white_point_saturates() {
    let mut scene = lit_cube(Vec::new());
    scene.ambient = [40.0; 3];
    let (image, _) = render(&scene, ViewMode::Lit);
    assert_eq!(pixel_at(&image, 0.0, 0.05), [255, 255, 255, 255]);
}

#[test]
fn gamma_changes_unlit_pixels_as_predicted() {
    let display = Display { gamma: 1.8 };
    let scene = near_and_far(true);
    let (image, _) = renderer()
        .render3(
            &scene,
            &scene.camera,
            ViewMode::Unlit,
            &display,
            WIDTH,
            HEIGHT,
        )
        .unwrap();
    let background = pixel_at(&image, 0.8, 0.05);
    let expected = |byte: u8| tone::encode_byte(tone::decode_byte(byte), 1.8);
    for (channel, byte) in background[..3].iter().zip(&BACKGROUND[..3]) {
        assert!(channel.abs_diff(expected(*byte)) <= 1, "{background:?}");
    }
    assert_ne!(background, BACKGROUND);
}

#[test]
fn tone_settings_must_be_finite_and_positive() {
    let renderer = renderer();
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut scene = lit_cube(Vec::new());
        scene.exposure = bad;
        assert!(
            renderer
                .render3(
                    &scene,
                    &scene.camera,
                    ViewMode::Lit,
                    &Display::default(),
                    WIDTH,
                    HEIGHT
                )
                .is_err()
        );
        let mut scene = lit_cube(Vec::new());
        scene.white_point = bad;
        assert!(
            renderer
                .render3(
                    &scene,
                    &scene.camera,
                    ViewMode::Lit,
                    &Display::default(),
                    WIDTH,
                    HEIGHT
                )
                .is_err()
        );
        let scene = lit_cube(Vec::new());
        assert!(
            renderer
                .render3(
                    &scene,
                    &scene.camera,
                    ViewMode::Lit,
                    &Display { gamma: bad },
                    WIDTH,
                    HEIGHT
                )
                .is_err()
        );
    }
}

#[test]
fn nearer_lamp_lights_the_face_brighter() {
    let (near, _) = render(&lit_cube(vec![lamp(200.0)]), ViewMode::Lit);
    let (far, _) = render(&lit_cube(vec![lamp(0.0)]), ViewMode::Lit);
    let (dark, _) = render(&lit_cube(Vec::new()), ViewMode::Lit);
    let at = |image: &Image| luma(pixel_at(image, 0.0, 0.05));
    assert!(at(&near) > at(&far), "{} vs {}", at(&near), at(&far));
    assert!(at(&far) > at(&dark), "{} vs {}", at(&far), at(&dark));
}

#[test]
fn lamp_behind_the_face_adds_nothing() {
    let (behind, _) = render(&lit_cube(vec![lamp(1000.0)]), ViewMode::Lit);
    let (dark, _) = render(&lit_cube(Vec::new()), ViewMode::Lit);
    assert_eq!(pixel_at(&behind, 0.0, 0.05), pixel_at(&dark, 0.0, 0.05));
}

#[test]
fn unlit_ignores_lamps() {
    let (lamps, _) = render(&lit_cube(vec![lamp(200.0)]), ViewMode::Unlit);
    assert_eq!(pixel_at(&lamps, 0.0, 0.05), [255, 255, 255, 255]);
}

#[test]
fn too_many_lamps_is_an_error() {
    let scene = lit_cube(vec![lamp(200.0); 17]);
    assert!(
        renderer()
            .render3(
                &scene,
                &scene.camera,
                ViewMode::Lit,
                &Display::default(),
                WIDTH,
                HEIGHT
            )
            .is_err()
    );
}

const WALL_X: f64 = 300.0;
const EYE: [f64; 3] = [0.0, 0.0, 50.0];
const AMBIENT: f64 = 0.25;

fn wall(spots: Vec<SpotLight>, lights: Vec<PointLight>) -> Scene3 {
    let mut wall = MeshInstance::new(cube(), [700.0, 0.0, -400.0]);
    wall.scale = 8.0;
    wall.color = Some([1.0, 1.0, 1.0, 1.0]);
    Scene3 {
        background: BACKGROUND,
        ambient: [AMBIENT as f32; 3],
        exposure: 1.0,
        white_point: 11.2,
        instances: vec![wall],
        lights,
        spots,
        textures: BTreeMap::new(),
        gobos: BTreeMap::new(),
        tick: 0,
        camera: eye(),
    }
}

fn spot_at_eye() -> SpotLight {
    let mut spot = SpotLight::new(EYE, Rotator::default());
    spot.intensity = 2.0;
    spot.attenuation_radius = 1000.0;
    spot
}

fn pixel_of(ndc_x: f64, ndc_y: f64) -> (u32, u32) {
    let x = ((ndc_x + 1.0) / 2.0 * f64::from(WIDTH)) as u32;
    let y = ((1.0 - ndc_y) / 2.0 * f64::from(HEIGHT)) as u32;
    (x, y)
}

fn wall_point(pixel: (u32, u32)) -> [f64; 3] {
    let ndc_x = 2.0 * (f64::from(pixel.0) + 0.5) / f64::from(WIDTH) - 1.0;
    let ndc_y = 1.0 - 2.0 * (f64::from(pixel.1) + 0.5) / f64::from(HEIGHT);
    let aspect = f64::from(WIDTH) / f64::from(HEIGHT);
    let depth = WALL_X - EYE[0];
    [
        WALL_X,
        EYE[1] + ndc_x * depth,
        EYE[2] + ndc_y * depth / aspect,
    ]
}

fn from_eye(point: [f64; 3]) -> (f64, f64) {
    let offset = [point[0] - EYE[0], point[1] - EYE[1], point[2] - EYE[2]];
    let distance = offset.iter().map(|v| v * v).sum::<f64>().sqrt();
    (distance, offset[0] / distance)
}

fn spot_light(spot: &SpotLight, point: [f64; 3], gobo: [f64; 3]) -> [f64; 3] {
    let (distance, cos_theta) = from_eye(point);
    let lambert = cos_theta;
    let falloff = light::distance_falloff(
        distance,
        spot.attenuation_radius,
        f64::from(spot.falloff_pow),
    );
    let shape = match spot.gobo {
        Some(_) => gobo,
        None => [light::spot_cone(cos_theta, spot.fov, f64::from(spot.spot_falloff_pow)); 3],
    };
    shape.map(|s| AMBIENT + f64::from(spot.intensity) * lambert * falloff * s)
}

fn expect_pixel(image: &Image, pixel: (u32, u32), linear: [f64; 3]) {
    let actual = image.pixel(pixel.0, pixel.1);
    for channel in 0..3 {
        let expected = tone_mapped(linear[channel], 1.0);
        assert!(
            actual[channel].abs_diff(expected) <= 1,
            "pixel {pixel:?} is {actual:?}, channel {channel} expected {expected}"
        );
    }
}

fn solid(color: [u8; 3]) -> Arc<Image> {
    let pixels = (0..16 * 16).flat_map(|_| [color[0], color[1], color[2], 255]);
    Arc::new(Image::new(16, 16, pixels.collect()).unwrap())
}

fn halves(left: [u8; 3], right: [u8; 3]) -> Arc<Image> {
    let pixels = (0..16 * 16).flat_map(|index| {
        let c = if index % 16 < 8 { left } else { right };
        [c[0], c[1], c[2], 255]
    });
    Arc::new(Image::new(16, 16, pixels.collect()).unwrap())
}

fn render_lit(scene: &Scene3) -> Image {
    render(scene, ViewMode::Lit).0
}

#[test]
fn spot_lights_its_pool_and_leaves_past_half_fov_at_ambient() {
    let spot = spot_at_eye();
    let image = render_lit(&wall(vec![spot.clone()], Vec::new()));
    let dark = render_lit(&wall(Vec::new(), Vec::new()));
    for ndc_x in [0.0, 0.2, -0.35] {
        let pixel = pixel_of(ndc_x, 0.05);
        expect_pixel(
            &image,
            pixel,
            spot_light(&spot, wall_point(pixel), [1.0; 3]),
        );
    }
    let centre = pixel_of(0.0, 0.05);
    assert!(luma(image.pixel(centre.0, centre.1)) > luma(dark.pixel(centre.0, centre.1)));
    for ndc_x in [0.8, -0.8] {
        let (x, y) = pixel_of(ndc_x, 0.05);
        assert_eq!(image.pixel(x, y), dark.pixel(x, y), "ndc {ndc_x}");
    }
}

#[test]
fn spot_falloff_power_tightens_the_pool() {
    let mut spot = spot_at_eye();
    spot.spot_falloff_pow = 4.0;
    let tight = render_lit(&wall(vec![spot.clone()], Vec::new()));
    let soft = render_lit(&wall(vec![spot_at_eye()], Vec::new()));
    let pixel = pixel_of(0.3, 0.05);
    expect_pixel(
        &tight,
        pixel,
        spot_light(&spot, wall_point(pixel), [1.0; 3]),
    );
    assert!(luma(tight.pixel(pixel.0, pixel.1)) < luma(soft.pixel(pixel.0, pixel.1)));
}

#[test]
fn gobo_tints_each_half_of_the_pool_as_the_reference_predicts() {
    let mut spot = spot_at_eye();
    spot.gobo = Some(GoboRef::still("halves"));
    let mut scene = wall(vec![spot.clone()], Vec::new());
    scene
        .gobos
        .insert("halves".to_string(), vec![halves([255, 0, 0], [0, 0, 255])]);
    let image = render_lit(&scene);
    let vp = spot.view_projection();
    for (ndc_x, colour) in [
        (-0.2, [1.0, 0.0, 0.0]),
        (0.2, [0.0, 0.0, 1.0]),
        (0.5, [0.0, 0.0, 1.0]),
    ] {
        let pixel = pixel_of(ndc_x, 0.05);
        let point = wall_point(pixel);
        assert!(light::gobo_uv(&vp, point).is_some(), "ndc {ndc_x}");
        expect_pixel(&image, pixel, spot_light(&spot, point, colour));
    }
    let outside = pixel_of(0.8, 0.05);
    assert!(light::gobo_uv(&vp, wall_point(outside)).is_none());
    expect_pixel(&image, outside, [AMBIENT; 3]);
}

#[test]
fn gobo_changes_frame_on_the_tick_the_reference_says() {
    let mut spot = spot_at_eye();
    spot.gobo = Some(GoboRef {
        name: "flicker".to_string(),
        anim: GoboAnim::Loop,
        frame_ticks: 6,
        start_tick: 0,
    });
    let mut scene = wall(vec![spot.clone()], Vec::new());
    scene.gobos.insert(
        "flicker".to_string(),
        vec![solid([255, 0, 0]), solid([0, 0, 255])],
    );
    let pixel = pixel_of(0.0, 0.05);
    let point = wall_point(pixel);
    for (tick, colour) in [
        (5, [1.0, 0.0, 0.0]),
        (6, [0.0, 0.0, 1.0]),
        (12, [1.0, 0.0, 0.0]),
    ] {
        scene.tick = tick;
        let frame = light::gobo_frame(GoboAnim::Loop, 2, 6, 0, tick);
        assert_eq!(colour[2] == 1.0, frame == 1, "tick {tick}");
        expect_pixel(&render_lit(&scene), pixel, spot_light(&spot, point, colour));
    }
}

#[test]
fn point_light_follows_the_hpl3_falloff() {
    for power in [1.0, 2.0] {
        let lamp = PointLight {
            location: EYE,
            color: [1.0; 3],
            intensity: 2.0,
            attenuation_radius: 1000.0,
            falloff_pow: power,
        };
        let image = render_lit(&wall(Vec::new(), vec![lamp]));
        for ndc_x in [0.0, 0.4] {
            let pixel = pixel_of(ndc_x, 0.05);
            let (distance, lambert) = from_eye(wall_point(pixel));
            let linear = AMBIENT + 2.0 * lambert * lamp.falloff(distance);
            expect_pixel(&image, pixel, [linear; 3]);
        }
    }
}

#[test]
fn bad_spots_and_gobos_are_errors() {
    let renderer = renderer();
    let fails = |scene: &Scene3| {
        renderer
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
    assert!(fails(&wall(vec![spot_at_eye(); 17], Vec::new())));
    let spoilers: [fn(&mut SpotLight); 9] = [
        |s| s.fov = 0.0,
        |s| s.fov = 180.0,
        |s| s.aspect = 0.0,
        |s| s.near_plane = f64::NAN,
        |s| s.attenuation_radius = -1.0,
        |s| s.falloff_pow = 0.0,
        |s| s.spot_falloff_pow = -2.0,
        |s| s.intensity = f32::INFINITY,
        |s| s.gobo = Some(GoboRef::still("missing")),
    ];
    for (index, spoil) in spoilers.iter().enumerate() {
        let mut spot = spot_at_eye();
        spoil(&mut spot);
        assert!(fails(&wall(vec![spot], Vec::new())), "case {index}");
    }
    let mut mixed = wall(Vec::new(), Vec::new());
    mixed.gobos.insert("a".to_string(), vec![solid([255; 3])]);
    let big = Arc::new(Image::new(32, 32, vec![255; 32 * 32 * 4]).unwrap());
    mixed.gobos.insert("b".to_string(), vec![big]);
    assert!(fails(&mixed));
    let mut many = wall(Vec::new(), Vec::new());
    many.gobos
        .insert("a".to_string(), vec![solid([255; 3]); 65]);
    assert!(fails(&many));
    let mut odd = wall(Vec::new(), Vec::new());
    let narrow = Arc::new(Image::new(24, 16, vec![255; 24 * 16 * 4]).unwrap());
    odd.gobos.insert("a".to_string(), vec![narrow]);
    assert!(fails(&odd));
}
