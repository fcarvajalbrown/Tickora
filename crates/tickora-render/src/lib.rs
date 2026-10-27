pub mod camera;
pub mod compare;
pub mod golden;
pub mod image;
pub mod math;
pub mod mesh;
pub mod renderer;
pub mod scene;
pub mod scene3;

pub use camera::{Camera, OrthoCamera, PerspectiveCamera, Rotator, Viewport};
pub use compare::{Comparison, Tolerance, compare};
pub use image::Image;
pub use mesh::{Mesh, Primitive};
pub use renderer::{AdapterChoice, AdapterInfo, Renderer};
pub use scene::{Color, Fill, Grid, Layout, PixelRect, Quad, Scene, ViewMode};
pub use scene3::{MeshInstance, Scene3};

pub const MIN_SIZE: u32 = 16;
pub const MAX_SIZE: u32 = 2048;
pub const DEFAULT_SIZE: u32 = 256;
