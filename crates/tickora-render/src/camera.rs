use std::fmt;
use std::str::FromStr;

use crate::math::{Mat4, Vec3};

pub const DEFAULT_ORTHO_WIDTH: f64 = 256.0;
pub const DEFAULT_FOV: f64 = 90.0;
pub const MIN_FOV: f64 = 5.0;
pub const MAX_FOV: f64 = 170.0;
pub const NEAR_PLANE: f64 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewport {
    Top,
    Perspective,
}

impl Viewport {
    pub fn name(self) -> &'static str {
        match self {
            Viewport::Top => "top",
            Viewport::Perspective => "perspective",
        }
    }
}

impl FromStr for Viewport {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text {
            "top" => Ok(Viewport::Top),
            "perspective" => Ok(Viewport::Perspective),
            "front" | "side" => Err(format!(
                "viewport {text:?} is reserved; expected \"top\" or \"perspective\""
            )),
            other => Err(format!(
                "unknown viewport {other:?}; expected \"top\" or \"perspective\""
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrthoCamera {
    pub location: [f64; 2],
    pub ortho_width: f64,
}

impl Default for OrthoCamera {
    fn default() -> Self {
        OrthoCamera {
            location: [0.0, 0.0],
            ortho_width: DEFAULT_ORTHO_WIDTH,
        }
    }
}

impl OrthoCamera {
    pub fn new(location: [f64; 2], ortho_width: f64) -> Result<Self, String> {
        if !location.iter().all(|v| v.is_finite()) {
            return Err("location must be finite numbers".to_string());
        }
        if !(ortho_width.is_finite() && ortho_width > 0.0) {
            return Err("ortho_width must be a finite number above 0".to_string());
        }
        Ok(OrthoCamera {
            location,
            ortho_width,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotator {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

impl Rotator {
    pub fn new(pitch: f64, yaw: f64, roll: f64) -> Self {
        Rotator { pitch, yaw, roll }
    }

    pub fn axes(&self) -> [Vec3; 3] {
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sr, cr) = self.roll.to_radians().sin_cos();
        let forward = [cp * cy, cp * sy, sp];
        let right = [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp];
        let up = [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp];
        [forward, right, up]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveCamera {
    pub location: Vec3,
    pub rotation: Rotator,
    pub fov: f64,
}

impl PerspectiveCamera {
    pub fn new(location: Vec3, rotation: Rotator, fov: f64) -> Result<Self, String> {
        if !location.iter().all(|v| v.is_finite()) {
            return Err("location must be finite numbers".to_string());
        }
        if ![rotation.pitch, rotation.yaw, rotation.roll]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err("rotation must be finite numbers".to_string());
        }
        if !(fov.is_finite() && (MIN_FOV..=MAX_FOV).contains(&fov)) {
            return Err(format!("fov must be from {MIN_FOV} to {MAX_FOV} degrees"));
        }
        Ok(PerspectiveCamera {
            location,
            rotation,
            fov,
        })
    }

    pub fn view(&self) -> Mat4 {
        let [forward, right, up] = self.rotation.axes();
        let eye = self.location;
        let row = |axis: Vec3| {
            let along = -(axis[0] * eye[0] + axis[1] * eye[1] + axis[2] * eye[2]);
            [axis[0], axis[1], axis[2], along]
        };
        Mat4::from_rows([row(right), row(up), row(forward), [0.0, 0.0, 0.0, 1.0]])
    }

    pub fn projection(&self, width: u32, height: u32) -> Mat4 {
        let half = (self.fov.to_radians() / 2.0).tan();
        let aspect = f64::from(width) / f64::from(height);
        Mat4::from_rows([
            [1.0 / half, 0.0, 0.0, 0.0],
            [0.0, aspect / half, 0.0, 0.0],
            [0.0, 0.0, 0.0, NEAR_PLANE],
            [0.0, 0.0, 1.0, 0.0],
        ])
    }

    pub fn view_projection(&self, width: u32, height: u32) -> Mat4 {
        self.projection(width, height).mul(&self.view())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Camera {
    Top(OrthoCamera),
    Perspective(PerspectiveCamera),
}

impl Default for Camera {
    fn default() -> Self {
        Camera::Top(OrthoCamera::default())
    }
}

impl Camera {
    pub fn viewport(&self) -> Viewport {
        match self {
            Camera::Top(_) => Viewport::Top,
            Camera::Perspective(_) => Viewport::Perspective,
        }
    }
}

impl fmt::Display for Camera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Camera::Top(camera) => write!(
                f,
                "top {} {} {}",
                camera.location[0], camera.location[1], camera.ortho_width
            ),
            Camera::Perspective(camera) => write!(
                f,
                "perspective {} {} {} {} {} {} {}",
                camera.location[0],
                camera.location[1],
                camera.location[2],
                camera.rotation.pitch,
                camera.rotation.yaw,
                camera.rotation.roll,
                camera.fov
            ),
        }
    }
}

impl FromStr for Camera {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let number = |part: &str| {
            part.parse::<f64>()
                .map_err(|_| format!("camera {text:?}: {part:?} is not a number"))
        };
        let Some((viewport, values)) = parts.split_first() else {
            return Err("camera string is empty".to_string());
        };
        let values = values
            .iter()
            .map(|part| number(part))
            .collect::<Result<Vec<f64>, String>>();
        match viewport.parse::<Viewport>()? {
            Viewport::Top => match values?.as_slice() {
                [x, y, width] => Ok(Camera::Top(OrthoCamera::new([*x, *y], *width)?)),
                _ => Err(format!(
                    "camera {text:?} must be \"top <x> <y> <ortho_width>\""
                )),
            },
            Viewport::Perspective => match values?.as_slice() {
                [x, y, z, pitch, yaw, roll, fov] => Ok(Camera::Perspective(
                    PerspectiveCamera::new([*x, *y, *z], Rotator::new(*pitch, *yaw, *roll), *fov)?,
                )),
                _ => Err(format!(
                    "camera {text:?} must be \"perspective <x> <y> <z> <pitch> <yaw> <roll> <fov>\""
                )),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f64; 4], b: [f64; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    fn perspective(location: Vec3, pitch: f64, yaw: f64, fov: f64) -> PerspectiveCamera {
        PerspectiveCamera::new(location, Rotator::new(pitch, yaw, 0.0), fov).unwrap()
    }

    #[test]
    fn default_prints_as_unreal_style_string() {
        assert_eq!(Camera::default().to_string(), "top 0 0 256");
    }

    #[test]
    fn top_string_round_trips_exactly() {
        let camera = Camera::Top(OrthoCamera::new([-12.375, 0.1], 300.5).unwrap());
        let back: Camera = camera.to_string().parse().unwrap();
        assert_eq!(back, camera);
    }

    #[test]
    fn perspective_string_round_trips_exactly() {
        let camera = Camera::Perspective(
            PerspectiveCamera::new([120.5, -3.25, 170.0], Rotator::new(-10.0, 45.5, 0.1), 90.0)
                .unwrap(),
        );
        let text = camera.to_string();
        assert_eq!(text, "perspective 120.5 -3.25 170 -10 45.5 0.1 90");
        assert_eq!(text.parse::<Camera>().unwrap(), camera);
    }

    #[test]
    fn rejects_bad_strings() {
        for bad in [
            "",
            "top 0 0",
            "top 0 0 0",
            "top 0 x 10",
            "top 0 NaN 10",
            "front 0 0 10",
            "left 0 0 10",
            "perspective 0 0 0 0 0 0",
            "perspective 0 0 0 0 0 0 4",
            "perspective 0 0 0 0 0 0 171",
            "perspective 0 0 inf 0 0 0 90",
            "perspective 0 0 0 0 NaN 0 90",
        ] {
            assert!(bad.parse::<Camera>().is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn zero_rotation_looks_down_x_with_y_right_and_z_up() {
        let [forward, right, up] = Rotator::default().axes();
        assert_eq!(forward, [1.0, 0.0, 0.0]);
        assert_eq!(right, [0.0, 1.0, 0.0]);
        assert_eq!(up, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn yaw_turns_from_x_towards_y_and_pitch_looks_up() {
        let [forward, right, _] = Rotator::new(0.0, 90.0, 0.0).axes();
        assert!(close(
            [forward[0], forward[1], forward[2], 0.0],
            [0.0, 1.0, 0.0, 0.0]
        ));
        assert!(close(
            [right[0], right[1], right[2], 0.0],
            [-1.0, 0.0, 0.0, 0.0]
        ));
        let [forward, _, up] = Rotator::new(90.0, 0.0, 0.0).axes();
        assert!(close(
            [forward[0], forward[1], forward[2], 0.0],
            [0.0, 0.0, 1.0, 0.0]
        ));
        assert!(close([up[0], up[1], up[2], 0.0], [-1.0, 0.0, 0.0, 0.0]));
    }

    #[test]
    fn view_moves_the_eye_to_the_origin() {
        let camera = perspective([100.0, 50.0, 170.0], 0.0, 90.0, 90.0);
        let view = camera.view();
        assert!(close(
            view.apply([100.0, 50.0, 170.0, 1.0]),
            [0.0, 0.0, 0.0, 1.0]
        ));
        assert!(close(
            view.apply([100.0, 150.0, 170.0, 1.0]),
            [0.0, 0.0, 100.0, 1.0]
        ));
        assert!(close(
            view.apply([0.0, 50.0, 170.0, 1.0]),
            [100.0, 0.0, 0.0, 1.0]
        ));
        assert!(close(
            view.apply([100.0, 50.0, 200.0, 1.0]),
            [0.0, 30.0, 0.0, 1.0]
        ));
    }

    #[test]
    fn projection_is_reversed_z_with_infinite_far_plane() {
        let camera = perspective([0.0, 0.0, 0.0], 0.0, 0.0, 90.0);
        let matrix = camera.view_projection(200, 100);
        let ndc = |point: Vec3| {
            let clip = matrix.apply([point[0], point[1], point[2], 1.0]);
            [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3], 1.0]
        };
        assert!(close(ndc([NEAR_PLANE, 0.0, 0.0]), [0.0, 0.0, 1.0, 1.0]));
        assert!(close(ndc([100.0, 0.0, 0.0]), [0.0, 0.0, 0.1, 1.0]));
        assert!(ndc([1.0e12, 0.0, 0.0])[2] < 1e-10);
        assert!(close(ndc([100.0, 100.0, 0.0]), [1.0, 0.0, 0.1, 1.0]));
        assert!(close(ndc([100.0, 0.0, 50.0]), [0.0, 1.0, 0.1, 1.0]));
    }
}
