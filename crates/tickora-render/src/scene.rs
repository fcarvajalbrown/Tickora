use crate::camera::Camera;

pub type Color = [u8; 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Solid,
    Outline,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub center: [f64; 2],
    pub half_extent: f64,
    pub color: Color,
    pub fill: Fill,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub spacing: f64,
    pub line: Color,
    pub axis: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub background: Color,
    pub grid: Option<Grid>,
    pub quads: Vec<Quad>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub rects: Vec<PixelRect>,
    pub off_screen: usize,
}

struct Projection {
    left: f64,
    top: f64,
    scale: f64,
    width: u32,
    height: u32,
}

impl Projection {
    fn new(camera: &Camera, width: u32, height: u32) -> Self {
        let scale = f64::from(width) / camera.ortho_width;
        let ortho_height = f64::from(height) / scale;
        Projection {
            left: camera.location[0] - camera.ortho_width / 2.0,
            top: camera.location[1] + ortho_height / 2.0,
            scale,
            width,
            height,
        }
    }

    fn x(&self, world: f64) -> f64 {
        (world - self.left) * self.scale
    }

    fn y(&self, world: f64) -> f64 {
        (self.top - world) * self.scale
    }

    fn clamp_x(&self, pixel: f64) -> u32 {
        pixel.clamp(0.0, f64::from(self.width)) as u32
    }

    fn clamp_y(&self, pixel: f64) -> u32 {
        pixel.clamp(0.0, f64::from(self.height)) as u32
    }
}

impl Scene {
    pub fn layout(&self, camera: &Camera, width: u32, height: u32) -> Layout {
        let projection = Projection::new(camera, width, height);
        let mut rects = Vec::new();
        if let Some(grid) = &self.grid {
            grid_rects(grid, &projection, &mut rects);
        }
        let mut off_screen = 0;
        for quad in &self.quads {
            match quad_rect(quad, &projection) {
                Some(rect) => outline_or_fill(rect, quad.fill, &mut rects),
                None => off_screen += 1,
            }
        }
        Layout { rects, off_screen }
    }
}

fn grid_rects(grid: &Grid, projection: &Projection, rects: &mut Vec<PixelRect>) {
    if !(grid.spacing.is_finite() && grid.spacing * projection.scale >= 2.0) {
        return;
    }
    let (width, height) = (projection.width, projection.height);
    let right = projection.left + f64::from(width) / projection.scale;
    let bottom = projection.top - f64::from(height) / projection.scale;
    let first = (projection.left / grid.spacing).ceil() as i64;
    let last = (right / grid.spacing).floor() as i64;
    for k in first..=last {
        let column = projection.x(k as f64 * grid.spacing).floor();
        if (0.0..f64::from(width)).contains(&column) {
            let x = column as u32;
            let color = if k == 0 { grid.axis } else { grid.line };
            rects.push(PixelRect {
                x0: x,
                y0: 0,
                x1: x + 1,
                y1: height,
                color,
            });
        }
    }
    let first = (bottom / grid.spacing).ceil() as i64;
    let last = (projection.top / grid.spacing).floor() as i64;
    for k in first..=last {
        let row = projection.y(k as f64 * grid.spacing).floor();
        if (0.0..f64::from(height)).contains(&row) {
            let y = row as u32;
            let color = if k == 0 { grid.axis } else { grid.line };
            rects.push(PixelRect {
                x0: 0,
                y0: y,
                x1: width,
                y1: y + 1,
                color,
            });
        }
    }
}

fn quad_rect(quad: &Quad, projection: &Projection) -> Option<PixelRect> {
    let [cx, cy] = quad.center;
    let mut left = projection.x(cx - quad.half_extent).round();
    let mut right = projection.x(cx + quad.half_extent).round();
    let mut top = projection.y(cy + quad.half_extent).round();
    let mut bottom = projection.y(cy - quad.half_extent).round();
    if right <= left {
        right = left + 1.0;
    }
    if bottom <= top {
        bottom = top + 1.0;
    }
    if right <= 0.0
        || bottom <= 0.0
        || left >= f64::from(projection.width)
        || top >= f64::from(projection.height)
    {
        return None;
    }
    left = left.max(0.0);
    top = top.max(0.0);
    Some(PixelRect {
        x0: projection.clamp_x(left),
        y0: projection.clamp_y(top),
        x1: projection.clamp_x(right),
        y1: projection.clamp_y(bottom),
        color: quad.color,
    })
}

fn outline_or_fill(rect: PixelRect, fill: Fill, rects: &mut Vec<PixelRect>) {
    let thin = rect.x1 - rect.x0 <= 2 || rect.y1 - rect.y0 <= 2;
    if fill == Fill::Solid || thin {
        rects.push(rect);
        return;
    }
    let PixelRect {
        x0,
        y0,
        x1,
        y1,
        color,
    } = rect;
    rects.push(PixelRect {
        x0,
        y0,
        x1,
        y1: y0 + 1,
        color,
    });
    rects.push(PixelRect {
        x0,
        y0: y1 - 1,
        x1,
        y1,
        color,
    });
    rects.push(PixelRect {
        x0,
        y0: y0 + 1,
        x1: x0 + 1,
        y1: y1 - 1,
        color,
    });
    rects.push(PixelRect {
        x0: x1 - 1,
        y0: y0 + 1,
        x1,
        y1: y1 - 1,
        color,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Viewport;

    const RED: Color = [255, 0, 0, 255];

    fn quad(x: f64, y: f64, fill: Fill) -> Quad {
        Quad {
            center: [x, y],
            half_extent: 2.0,
            color: RED,
            fill,
        }
    }

    fn scene(quads: Vec<Quad>) -> Scene {
        Scene {
            background: [0, 0, 0, 255],
            grid: None,
            quads,
        }
    }

    #[test]
    fn origin_square_lands_in_the_centre() {
        let layout = scene(vec![quad(0.0, 0.0, Fill::Solid)]).layout(&Camera::default(), 256, 256);
        assert_eq!(
            layout.rects,
            vec![PixelRect {
                x0: 126,
                y0: 126,
                x1: 130,
                y1: 130,
                color: RED
            }]
        );
        assert_eq!(layout.off_screen, 0);
    }

    #[test]
    fn plus_y_is_up_and_plus_x_is_right() {
        let layout =
            scene(vec![quad(10.0, 20.0, Fill::Solid)]).layout(&Camera::default(), 256, 256);
        let rect = layout.rects[0];
        assert_eq!((rect.x0, rect.y0), (136, 106));
    }

    #[test]
    fn camera_location_and_width_move_the_view() {
        let camera = Camera::new(Viewport::Top, [10.0, 20.0], 128.0).unwrap();
        let layout = scene(vec![quad(10.0, 20.0, Fill::Solid)]).layout(&camera, 256, 256);
        assert_eq!(
            layout.rects[0],
            PixelRect {
                x0: 124,
                y0: 124,
                x1: 132,
                y1: 132,
                color: RED
            }
        );
    }

    #[test]
    fn far_quads_are_counted_off_screen() {
        let layout = scene(vec![
            quad(500.0, 0.0, Fill::Solid),
            quad(0.0, -500.0, Fill::Solid),
        ])
        .layout(&Camera::default(), 256, 256);
        assert!(layout.rects.is_empty());
        assert_eq!(layout.off_screen, 2);
    }

    #[test]
    fn edge_quads_are_clipped_not_dropped() {
        let layout =
            scene(vec![quad(-128.0, 0.0, Fill::Solid)]).layout(&Camera::default(), 256, 256);
        assert_eq!(layout.rects[0].x0, 0);
        assert_eq!(layout.rects[0].x1, 2);
        assert_eq!(layout.off_screen, 0);
    }

    #[test]
    fn tiny_quads_keep_one_pixel() {
        let camera = Camera::new(Viewport::Top, [0.0, 0.0], 100_000.0).unwrap();
        let layout = scene(vec![quad(0.0, 0.0, Fill::Solid)]).layout(&camera, 256, 256);
        let rect = layout.rects[0];
        assert_eq!((rect.x1 - rect.x0, rect.y1 - rect.y0), (1, 1));
    }

    #[test]
    fn outline_is_four_one_pixel_edges() {
        let layout =
            scene(vec![quad(0.0, 0.0, Fill::Outline)]).layout(&Camera::default(), 256, 256);
        assert_eq!(layout.rects.len(), 4);
        let area: u32 = layout
            .rects
            .iter()
            .map(|r| (r.x1 - r.x0) * (r.y1 - r.y0))
            .sum();
        assert_eq!(area, 4 * 4 - 2 * 2);
    }

    #[test]
    fn grid_draws_axes_in_axis_colour() {
        let grid = Grid {
            spacing: 32.0,
            line: [1, 1, 1, 255],
            axis: [9, 9, 9, 255],
        };
        let scene = Scene {
            background: [0, 0, 0, 255],
            grid: Some(grid),
            quads: vec![],
        };
        let layout = scene.layout(&Camera::default(), 256, 256);
        assert_eq!(layout.rects.len(), 8 + 8);
        let axes: Vec<&PixelRect> = layout
            .rects
            .iter()
            .filter(|r| r.color == grid.axis)
            .collect();
        assert_eq!(axes.len(), 2);
        assert_eq!((axes[0].x0, axes[0].x1), (128, 129));
        assert_eq!((axes[1].y0, axes[1].y1), (128, 129));
    }

    #[test]
    fn dense_grid_is_skipped() {
        let grid = Grid {
            spacing: 1.0,
            line: [1, 1, 1, 255],
            axis: [9, 9, 9, 255],
        };
        let scene = Scene {
            background: [0, 0, 0, 255],
            grid: Some(grid),
            quads: vec![],
        };
        let camera = Camera::new(Viewport::Top, [0.0, 0.0], 1024.0).unwrap();
        assert!(scene.layout(&camera, 256, 256).rects.is_empty());
    }
}
