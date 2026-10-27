use std::fmt;
use std::str::FromStr;

pub const DEFAULT_ORTHO_WIDTH: f64 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewport {
    Top,
}

impl Viewport {
    pub fn name(self) -> &'static str {
        match self {
            Viewport::Top => "top",
        }
    }
}

impl FromStr for Viewport {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text {
            "top" => Ok(Viewport::Top),
            "perspective" | "front" | "side" => Err(format!(
                "viewport {text:?} is reserved for 3D scenes; only \"top\" is available"
            )),
            other => Err(format!("unknown viewport {other:?}; expected \"top\"")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub viewport: Viewport,
    pub location: [f64; 2],
    pub ortho_width: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            viewport: Viewport::Top,
            location: [0.0, 0.0],
            ortho_width: DEFAULT_ORTHO_WIDTH,
        }
    }
}

impl Camera {
    pub fn new(viewport: Viewport, location: [f64; 2], ortho_width: f64) -> Result<Self, String> {
        if !location.iter().all(|v| v.is_finite()) {
            return Err("location must be finite numbers".to_string());
        }
        if !(ortho_width.is_finite() && ortho_width > 0.0) {
            return Err("ortho_width must be a finite number above 0".to_string());
        }
        Ok(Camera {
            viewport,
            location,
            ortho_width,
        })
    }
}

impl fmt::Display for Camera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} {}",
            self.viewport.name(),
            self.location[0],
            self.location[1],
            self.ortho_width
        )
    }
}

impl FromStr for Camera {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let [viewport, x, y, width] = parts.as_slice() else {
            return Err(format!(
                "camera {text:?} must be \"<viewport> <x> <y> <ortho_width>\""
            ));
        };
        let number = |part: &str| {
            part.parse::<f64>()
                .map_err(|_| format!("camera {text:?}: {part:?} is not a number"))
        };
        Camera::new(viewport.parse()?, [number(x)?, number(y)?], number(width)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_prints_as_unreal_style_string() {
        assert_eq!(Camera::default().to_string(), "top 0 0 256");
    }

    #[test]
    fn string_round_trips_exactly() {
        let camera = Camera::new(Viewport::Top, [-12.375, 0.1], 300.5).unwrap();
        let back: Camera = camera.to_string().parse().unwrap();
        assert_eq!(back, camera);
    }

    #[test]
    fn rejects_bad_strings() {
        assert!("top 0 0".parse::<Camera>().is_err());
        assert!("top 0 0 0".parse::<Camera>().is_err());
        assert!("top 0 x 10".parse::<Camera>().is_err());
        assert!("top 0 NaN 10".parse::<Camera>().is_err());
        assert!("perspective 0 0 10".parse::<Camera>().is_err());
        assert!("left 0 0 10".parse::<Camera>().is_err());
    }
}
