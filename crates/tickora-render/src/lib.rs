pub mod camera;
pub mod image;
pub mod renderer;
pub mod scene;

pub use camera::{Camera, Viewport};
pub use image::Image;
pub use renderer::{AdapterChoice, AdapterInfo, Renderer};
pub use scene::{Color, Fill, Grid, Layout, PixelRect, Quad, Scene};

pub const MIN_SIZE: u32 = 16;
pub const MAX_SIZE: u32 = 2048;
pub const DEFAULT_SIZE: u32 = 256;
