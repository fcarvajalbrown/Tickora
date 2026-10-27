use std::sync::OnceLock;

use serde_json::{Value, json};
use tickora_render::{
    AdapterChoice, Camera, DEFAULT_SIZE, Image, OrthoCamera, Renderer, ViewMode, Viewport,
};

use crate::host::Host;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub camera: Camera,
    pub width: u32,
    pub height: u32,
    pub view_mode: ViewMode,
    pub show_grid: bool,
}

impl Default for Shot {
    fn default() -> Self {
        Shot {
            camera: Camera::default(),
            width: DEFAULT_SIZE,
            height: DEFAULT_SIZE,
            view_mode: ViewMode::Unlit,
            show_grid: true,
        }
    }
}

pub struct Capture {
    pub image: Image,
    pub off_screen: usize,
    pub bucket: String,
    pub adapter: Value,
}

pub fn renderer() -> Result<&'static Renderer, String> {
    static RENDERER: OnceLock<Result<Renderer, String>> = OnceLock::new();
    RENDERER
        .get_or_init(|| AdapterChoice::from_env().and_then(Renderer::new))
        .as_ref()
        .map_err(Clone::clone)
}

pub fn capture<H: Host>(host: &H, shot: &Shot) -> Result<Capture, String> {
    let Camera::Top(camera) = &shot.camera else {
        return Err(PERSPECTIVE_PENDING.to_string());
    };
    let renderer = renderer()?;
    let mut scene = host.scene(shot.view_mode);
    if !shot.show_grid {
        scene.grid = None;
    }
    let (image, layout) = renderer.render(&scene, camera, shot.width, shot.height)?;
    let info = renderer.info();
    Ok(Capture {
        image,
        off_screen: layout.off_screen,
        bucket: info.bucket(),
        adapter: json!({
            "name": info.name,
            "backend": info.backend,
            "device_type": info.device_type,
        }),
    })
}

const PERSPECTIVE_PENDING: &str =
    "the perspective viewport is not wired to the simulation yet; use \"top\"";

pub const SHOT_KEYS: [&str; 8] = [
    "viewport",
    "location",
    "ortho_width",
    "camera",
    "width",
    "height",
    "view_mode",
    "show_grid",
];

pub fn from_args(args: &serde_json::Map<String, Value>) -> Result<Shot, String> {
    let defaults = Shot::default();
    let text = |key: &str| -> Result<Option<&str>, String> {
        match args.get(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text)),
            Some(_) => Err(format!("argument {key:?} must be a string")),
        }
    };
    let number = |key: &str| -> Result<Option<f64>, String> {
        match args.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_f64()
                .map(Some)
                .ok_or_else(|| format!("argument {key:?} must be a number")),
        }
    };
    let size = |key: &str| -> Result<u32, String> {
        match args.get(key) {
            None => Ok(DEFAULT_SIZE),
            Some(value) => value
                .as_u64()
                .and_then(|size| u32::try_from(size).ok())
                .ok_or_else(|| format!("argument {key:?} must be a whole number")),
        }
    };
    let camera = match text("camera")? {
        Some(camera) => {
            if ["viewport", "location", "ortho_width"]
                .iter()
                .any(|key| args.contains_key(*key))
            {
                return Err(
                    "give either camera or viewport, location and ortho_width, not both"
                        .to_string(),
                );
            }
            camera.parse()?
        }
        None => {
            let viewport = match text("viewport")? {
                Some(name) => name.parse()?,
                None => defaults.camera.viewport(),
            };
            if viewport == Viewport::Perspective {
                return Err(PERSPECTIVE_PENDING.to_string());
            }
            let top = OrthoCamera::default();
            let location = match args.get("location") {
                None => top.location,
                Some(Value::Array(items)) if items.len() == 2 => {
                    let mut out = [0.0; 2];
                    for (slot, item) in out.iter_mut().zip(items) {
                        *slot = item
                            .as_f64()
                            .ok_or("argument \"location\" must be two numbers")?;
                    }
                    out
                }
                Some(_) => return Err("argument \"location\" must be two numbers".to_string()),
            };
            let ortho_width = number("ortho_width")?.unwrap_or(top.ortho_width);
            Camera::Top(OrthoCamera::new(location, ortho_width)?)
        }
    };
    let view_mode = match text("view_mode")? {
        Some(name) => name.parse()?,
        None => defaults.view_mode,
    };
    if view_mode == ViewMode::Lit && camera.viewport() == Viewport::Top {
        return Err("view_mode \"lit\" needs the perspective viewport".to_string());
    }
    let show_grid = match args.get("show_grid") {
        None => defaults.show_grid,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => return Err("argument \"show_grid\" must be a boolean".to_string()),
    };
    Ok(Shot {
        camera,
        width: size("width")?,
        height: size("height")?,
        view_mode,
        show_grid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(value: Value) -> serde_json::Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn defaults_are_top_view_256_unlit_with_grid() {
        assert_eq!(from_args(&args(json!({}))).unwrap(), Shot::default());
        assert_eq!(Shot::default().camera.to_string(), "top 0 0 256");
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
            parts.camera,
            Camera::Top(OrthoCamera::new([4.5, -2.0], 64.0).unwrap())
        );
    }

    #[test]
    fn rejects_bad_arguments() {
        for bad in [
            json!({ "camera": "top 0 0 10", "ortho_width": 5 }),
            json!({ "viewport": "front" }),
            json!({ "viewport": "perspective" }),
            json!({ "location": [1] }),
            json!({ "location": [1, "a"] }),
            json!({ "ortho_width": 0 }),
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
