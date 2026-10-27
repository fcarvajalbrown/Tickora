use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tickora_render::{
    AdapterChoice, Image, Mesh, MeshInstance, PerspectiveCamera, PointLight, Renderer, Rotator,
    Scene3, ViewMode,
};

const BACKGROUND: [u8; 4] = [24, 24, 28, 255];
const STONE: [u8; 4] = [120, 118, 112, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const WIDTH: u32 = 200;
const HEIGHT: u32 = 100;

fn renderer() -> Renderer {
    let choice = AdapterChoice::from_env().unwrap();
    Renderer::new(choice).unwrap_or_else(|e| panic!("no adapter for render tests: {e}"))
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
        instances,
        lights: Vec::new(),
        textures: BTreeMap::new(),
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
        .render3(scene, &scene.camera, view_mode, WIDTH, HEIGHT)
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
            .render3(&scene, &scene.camera, ViewMode::Unlit, 15, 64)
            .is_err()
    );
}

#[test]
fn plus_y_is_screen_right_and_plus_z_is_screen_up() {
    let scene = |location: [f64; 3]| Scene3 {
        background: BACKGROUND,
        ambient: [0.0; 3],
        instances: vec![MeshInstance::new(cube(), location)],
        lights: Vec::new(),
        textures: BTreeMap::new(),
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
        instances: vec![cube],
        lights: Vec::new(),
        textures: BTreeMap::from([("stone".to_string(), Arc::new(checker))]),
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
        instances: vec![cube],
        lights,
        textures: BTreeMap::new(),
        camera: eye(),
    }
}

fn lamp(x: f64) -> PointLight {
    PointLight {
        location: [x, 0.0, 50.0],
        color: [1.0, 1.0, 1.0],
        intensity: 1.0,
        attenuation_radius: 1000.0,
    }
}

fn luma(pixel: [u8; 4]) -> u32 {
    u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2])
}

#[test]
fn lit_ambient_alone_scales_the_colour() {
    let (image, _) = render(&lit_cube(Vec::new()), ViewMode::Lit);
    let centre = pixel_at(&image, 0.0, 0.05);
    for channel in &centre[..3] {
        assert!((63..=64).contains(channel), "{centre:?}");
    }
    assert_eq!(centre[3], 255);
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
            .render3(&scene, &scene.camera, ViewMode::Lit, WIDTH, HEIGHT)
            .is_err()
    );
}
