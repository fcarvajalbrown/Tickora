use tickora_render::{AdapterChoice, Fill, Grid, OrthoCamera, Quad, Renderer, Scene};

const BACKGROUND: [u8; 4] = [24, 24, 28, 255];
const LINE: [u8; 4] = [48, 48, 56, 255];
const AXIS: [u8; 4] = [96, 96, 110, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

fn renderer() -> Renderer {
    let choice = AdapterChoice::from_env().unwrap();
    Renderer::new(choice).unwrap_or_else(|e| panic!("no adapter for render tests: {e}"))
}

fn scene() -> Scene {
    Scene {
        background: BACKGROUND,
        grid: Some(Grid {
            spacing: 32.0,
            line: LINE,
            axis: AXIS,
        }),
        quads: vec![
            Quad {
                center: [16.0, 16.0],
                half_extent: 2.0,
                color: RED,
                fill: Fill::Solid,
            },
            Quad {
                center: [-48.0, -48.0],
                half_extent: 8.0,
                color: GREEN,
                fill: Fill::Outline,
            },
            Quad {
                center: [900.0, 0.0],
                half_extent: 2.0,
                color: RED,
                fill: Fill::Solid,
            },
        ],
    }
}

#[test]
fn pixels_land_where_the_layout_says() {
    let renderer = renderer();
    let (image, layout) = renderer
        .render(&scene(), &OrthoCamera::default(), 256, 256)
        .unwrap();
    assert_eq!((image.width, image.height), (256, 256));
    assert_eq!(layout.off_screen, 1);
    assert_eq!(image.pixel(10, 10), BACKGROUND);
    assert_eq!(image.pixel(128, 10), AXIS);
    assert_eq!(image.pixel(10, 128), AXIS);
    assert_eq!(image.pixel(160, 10), LINE);
    assert_eq!(image.pixel(142, 110), RED);
    assert_eq!(image.pixel(145, 113), RED);
    assert_eq!(image.pixel(146, 113), BACKGROUND);
    assert_eq!(image.pixel(145, 114), BACKGROUND);
    assert_eq!(image.pixel(141, 110), BACKGROUND);
    assert_eq!(image.pixel(72, 168), GREEN);
    assert_eq!(image.pixel(87, 183), GREEN);
    assert_eq!(image.pixel(80, 176), BACKGROUND);
    assert_eq!(image.pixel(88, 176), BACKGROUND);
}

#[test]
fn same_scene_gives_the_same_pixels() {
    let renderer = renderer();
    let first = renderer
        .render(&scene(), &OrthoCamera::default(), 200, 120)
        .unwrap()
        .0;
    let second = renderer
        .render(&scene(), &OrthoCamera::default(), 200, 120)
        .unwrap()
        .0;
    assert_eq!(first.pixel_hash(), second.pixel_hash());
}

#[test]
fn empty_scene_is_all_background() {
    let renderer = renderer();
    let empty = Scene {
        background: BACKGROUND,
        grid: None,
        quads: vec![],
    };
    let (image, _) = renderer
        .render(&empty, &OrthoCamera::default(), 17, 33)
        .unwrap();
    assert!(image.pixels.chunks(4).all(|pixel| pixel == BACKGROUND));
}

#[test]
fn backend_matches_the_platform() {
    let renderer = renderer();
    let expected = if cfg!(windows) { "dx12" } else { "vulkan" };
    assert_eq!(renderer.info().backend, expected);
    assert_eq!(
        renderer.info().bucket(),
        format!("{}_{expected}", std::env::consts::OS)
    );
}

#[test]
fn rejects_sizes_out_of_range() {
    let renderer = renderer();
    assert!(
        renderer
            .render(&scene(), &OrthoCamera::default(), 15, 64)
            .is_err()
    );
    assert!(
        renderer
            .render(&scene(), &OrthoCamera::default(), 64, 2049)
            .is_err()
    );
}
