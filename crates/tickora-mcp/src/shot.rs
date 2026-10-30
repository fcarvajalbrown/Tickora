use std::sync::OnceLock;

use serde_json::{Map, Value, json};
use tickora_render::camera::DEFAULT_FOV;
use tickora_render::math::Vec3;
use tickora_render::tone;
use tickora_render::{
    AdapterChoice, Camera, DEFAULT_SIZE, Display, Image, OrthoCamera, PerspectiveCamera, Renderer,
    Rotator, ViewMode, Viewport,
};

use crate::host::Host;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Top(OrthoCamera),
    Perspective {
        location: Option<Vec3>,
        rotation: Option<Rotator>,
        fov: Option<f64>,
    },
}

impl View {
    pub fn viewport(&self) -> Viewport {
        match self {
            View::Top(_) => Viewport::Top,
            View::Perspective { .. } => Viewport::Perspective,
        }
    }

    fn from_camera(camera: Camera) -> View {
        match camera {
            Camera::Top(camera) => View::Top(camera),
            Camera::Perspective(camera) => View::Perspective {
                location: Some(camera.location),
                rotation: Some(camera.rotation),
                fov: Some(camera.fov),
            },
        }
    }

    fn perspective(&self, default: &PerspectiveCamera) -> Result<PerspectiveCamera, String> {
        match *self {
            View::Top(_) => Err("not a perspective view".to_string()),
            View::Perspective {
                location,
                rotation,
                fov,
            } => PerspectiveCamera::new(
                location.unwrap_or(default.location),
                rotation.unwrap_or(default.rotation),
                fov.unwrap_or(default.fov),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub view: View,
    pub width: u32,
    pub height: u32,
    pub view_mode: ViewMode,
    pub show_grid: bool,
    pub display: Display,
}

impl Default for Shot {
    fn default() -> Self {
        Shot {
            view: View::Top(OrthoCamera::default()),
            width: DEFAULT_SIZE,
            height: DEFAULT_SIZE,
            view_mode: ViewMode::Unlit,
            show_grid: true,
            display: Display::default(),
        }
    }
}

pub struct Capture {
    pub image: Image,
    pub camera: Camera,
    pub off_screen: usize,
    pub bucket: String,
    pub adapter: Value,
    pub details: Option<Value>,
}

pub fn renderer() -> Result<&'static Renderer, String> {
    static RENDERER: OnceLock<Result<Renderer, String>> = OnceLock::new();
    RENDERER
        .get_or_init(|| AdapterChoice::from_env().and_then(Renderer::new))
        .as_ref()
        .map_err(Clone::clone)
}

pub fn capture<H: Host>(host: &H, shot: &Shot) -> Result<Capture, String> {
    let renderer = renderer()?;
    let (image, camera, off_screen, details) = match &shot.view {
        View::Top(camera) => {
            let mut scene = host.scene(shot.view_mode);
            if !shot.show_grid {
                scene.grid = None;
            }
            let (image, layout) = renderer.render(&scene, camera, shot.width, shot.height)?;
            (image, Camera::Top(*camera), layout.off_screen, None)
        }
        view @ View::Perspective { .. } => {
            let scene = host.scene3();
            let camera = view.perspective(&scene.camera)?;
            let (image, off_screen) = renderer.render3(
                &scene,
                &camera,
                shot.view_mode,
                &shot.display,
                shot.width,
                shot.height,
            )?;
            let frames = scene.gobo_layers()?.frames.len();
            let details = json!({
                "exposure": scene.exposure,
                "white_point": scene.white_point,
                "gamma": shot.display.gamma,
                "point_lights": scene.lights.len(),
                "spot_lights": scene.spots.len(),
                "gobo_frames": frames,
            });
            (
                image,
                Camera::Perspective(camera),
                off_screen,
                Some(details),
            )
        }
    };
    let info = renderer.info();
    Ok(Capture {
        image,
        camera,
        off_screen,
        bucket: info.bucket(),
        adapter: json!({
            "name": info.name,
            "backend": info.backend,
            "device_type": info.device_type,
        }),
        details,
    })
}

pub const SHOT_KEYS: [&str; 11] = [
    "viewport",
    "location",
    "rotation",
    "fov",
    "ortho_width",
    "camera",
    "width",
    "height",
    "view_mode",
    "show_grid",
    "gamma",
];

const CAMERA_PARTS: [&str; 5] = ["viewport", "location", "rotation", "fov", "ortho_width"];

pub fn from_args(args: &Map<String, Value>) -> Result<Shot, String> {
    let view = match text(args, "camera")? {
        Some(camera) => {
            if CAMERA_PARTS.iter().any(|key| args.contains_key(*key)) {
                return Err(
                    "give either camera or viewport, location, rotation, fov and ortho_width, not both"
                        .to_string(),
                );
            }
            View::from_camera(camera.parse()?)
        }
        None => match text(args, "viewport")? {
            Some(name) => view_from_parts(args, name.parse()?)?,
            None => view_from_parts(args, Viewport::Top)?,
        },
    };
    let viewport = view.viewport();
    let view_mode = match text(args, "view_mode")? {
        Some(name) => name.parse()?,
        None if viewport == Viewport::Perspective => ViewMode::Lit,
        None => ViewMode::Unlit,
    };
    if view_mode == ViewMode::Lit && viewport == Viewport::Top {
        return Err("view_mode \"lit\" needs the perspective viewport".to_string());
    }
    let show_grid = match (args.get("show_grid"), viewport) {
        (None, Viewport::Top) => true,
        (None, _) => false,
        (Some(_), Viewport::Perspective) => {
            return Err("argument \"show_grid\" is only for the top viewport".to_string());
        }
        (Some(Value::Bool(flag)), _) => *flag,
        (Some(_), _) => return Err("argument \"show_grid\" must be a boolean".to_string()),
    };
    let display = match (number(args, "gamma")?, viewport) {
        (None, _) => Display::default(),
        (Some(_), Viewport::Top) => {
            return Err("argument \"gamma\" is only for the perspective viewport".to_string());
        }
        (Some(gamma), _) => {
            tone::check_positive("gamma", gamma)?;
            Display {
                gamma: gamma as f32,
                ..Display::default()
            }
        }
    };
    Ok(Shot {
        view,
        width: size(args, "width")?,
        height: size(args, "height")?,
        view_mode,
        show_grid,
        display,
    })
}

fn view_from_parts(args: &Map<String, Value>, viewport: Viewport) -> Result<View, String> {
    let only_for =
        |keys: &[&str], name: &str| match keys.iter().find(|key| args.contains_key(**key)) {
            Some(key) => Err(format!("argument {key:?} is only for the {name} viewport")),
            None => Ok(()),
        };
    match viewport {
        Viewport::Top => {
            only_for(&["rotation", "fov"], "perspective")?;
            let top = OrthoCamera::default();
            let location = numbers::<2>(args, "location")?.unwrap_or(top.location);
            let ortho_width = number(args, "ortho_width")?.unwrap_or(top.ortho_width);
            Ok(View::Top(OrthoCamera::new(location, ortho_width)?))
        }
        Viewport::Perspective => {
            only_for(&["ortho_width"], "top")?;
            let location = numbers::<3>(args, "location")?;
            let rotation = numbers::<3>(args, "rotation")?
                .map(|[pitch, yaw, roll]| Rotator::new(pitch, yaw, roll));
            let fov = number(args, "fov")?;
            PerspectiveCamera::new(
                location.unwrap_or_default(),
                rotation.unwrap_or_default(),
                fov.unwrap_or(DEFAULT_FOV),
            )?;
            Ok(View::Perspective {
                location,
                rotation,
                fov,
            })
        }
    }
}

fn text<'a>(args: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, String> {
    match args.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(format!("argument {key:?} must be a string")),
    }
}

fn number(args: &Map<String, Value>, key: &str) -> Result<Option<f64>, String> {
    match args.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or_else(|| format!("argument {key:?} must be a number")),
    }
}

fn numbers<const N: usize>(
    args: &Map<String, Value>,
    key: &str,
) -> Result<Option<[f64; N]>, String> {
    let wrong = || format!("argument {key:?} must be {N} numbers");
    match args.get(key) {
        None => Ok(None),
        Some(Value::Array(items)) if items.len() == N => {
            let mut out = [0.0; N];
            for (slot, item) in out.iter_mut().zip(items) {
                *slot = item.as_f64().ok_or_else(wrong)?;
            }
            Ok(Some(out))
        }
        Some(_) => Err(wrong()),
    }
}

fn size(args: &Map<String, Value>, key: &str) -> Result<u32, String> {
    match args.get(key) {
        None => Ok(DEFAULT_SIZE),
        Some(value) => value
            .as_u64()
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| format!("argument {key:?} must be a whole number")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn defaults_are_top_view_256_unlit_with_grid() {
        assert_eq!(from_args(&args(json!({}))).unwrap(), Shot::default());
        assert_eq!(
            Shot::default().view,
            View::Top(OrthoCamera::new([0.0, 0.0], 256.0).unwrap())
        );
    }

    #[test]
    fn camera_string_and_parts_give_the_same_shot() {
        let parts = from_args(&args(json!({
            "viewport": "top", "location": [4.5, -2], "ortho_width": 64
        })))
        .unwrap();
        let string = from_args(&args(json!({ "camera": "top 4.5 -2 64" }))).unwrap();
        assert_eq!(parts, string);
        assert_eq!(
            parts.view,
            View::Top(OrthoCamera::new([4.5, -2.0], 64.0).unwrap())
        );
    }

    #[test]
    fn perspective_string_and_parts_give_the_same_shot() {
        let parts = from_args(&args(json!({
            "viewport": "perspective", "location": [1, 2, 170], "rotation": [-10, 45, 0], "fov": 70
        })))
        .unwrap();
        let string = from_args(&args(
            json!({ "camera": "perspective 1 2 170 -10 45 0 70" }),
        ))
        .unwrap();
        assert_eq!(parts, string);
        assert_eq!(parts.view_mode, ViewMode::Lit);
        assert!(!parts.show_grid);
    }

    #[test]
    fn missing_perspective_parts_come_from_the_host_default() {
        let shot = from_args(&args(json!({ "viewport": "perspective", "fov": 60 }))).unwrap();
        let default = PerspectiveCamera::new([-5.0, 0.0, 170.0], Rotator::default(), 90.0).unwrap();
        let camera = shot.view.perspective(&default).unwrap();
        assert_eq!(camera.location, default.location);
        assert_eq!(camera.rotation, default.rotation);
        assert_eq!(camera.fov, 60.0);
    }

    #[test]
    fn rejects_bad_arguments() {
        for bad in [
            json!({ "camera": "top 0 0 10", "ortho_width": 5 }),
            json!({ "camera": "perspective 0 0 0 0 0 0 90", "fov": 60 }),
            json!({ "viewport": "front" }),
            json!({ "location": [1] }),
            json!({ "location": [1, "a"] }),
            json!({ "location": [1, 2, 3] }),
            json!({ "ortho_width": 0 }),
            json!({ "rotation": [0, 0, 0] }),
            json!({ "fov": 60 }),
            json!({ "viewport": "perspective", "location": [1, 2] }),
            json!({ "viewport": "perspective", "rotation": [0, 0] }),
            json!({ "viewport": "perspective", "fov": 4 }),
            json!({ "viewport": "perspective", "fov": 171 }),
            json!({ "viewport": "perspective", "ortho_width": 64 }),
            json!({ "viewport": "perspective", "show_grid": false }),
            json!({ "width": -4 }),
            json!({ "view_mode": "lit" }),
            json!({ "show_grid": "yes" }),
            json!({ "camera": 3 }),
        ] {
            assert!(from_args(&args(bad.clone())).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn reads_size_mode_and_grid() {
        let shot = from_args(&args(json!({
            "width": 64, "height": 32, "view_mode": "wireframe", "show_grid": false
        })))
        .unwrap();
        assert_eq!((shot.width, shot.height), (64, 32));
        assert_eq!(shot.view_mode, ViewMode::Wireframe);
        assert!(!shot.show_grid);
    }
}
