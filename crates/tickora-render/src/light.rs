use crate::camera::Rotator;
use crate::math::{Mat4, Vec3};

pub const DEFAULT_FALLOFF_POW: f64 = 1.0;
pub const DEFAULT_SPOT_FOV: f64 = 60.0;
pub const DEFAULT_SPOT_ASPECT: f64 = 1.0;
pub const DEFAULT_SPOT_NEAR_PLANE: f64 = 10.0;
pub const DEFAULT_SPOT_FALLOFF_POW: f64 = 1.0;
pub const MIN_SPOT_FOV: f64 = 1.0;
pub const MAX_SPOT_FOV: f64 = 179.0;
pub const SHADOW_NORMAL_OFFSET_CM: f64 = 0.5;
pub const SHADOW_DEPTH_BIAS: f64 = 0.5;
pub const SHADOW_SLOPE_BIAS: f64 = 1.0;
pub const DEPTH_MANTISSA_BITS: i32 = 23;
pub const CM_PER_M: f64 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GoboAnim {
    #[default]
    None,
    Loop,
    Oscillate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShadowResolution {
    Low,
    Medium,
    #[default]
    High,
}

impl ShadowResolution {
    pub fn side(self) -> u32 {
        match self {
            Self::Low => 256,
            Self::Medium => 512,
            Self::High => 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShadowQuality {
    Low,
    #[default]
    Medium,
    High,
}

impl ShadowQuality {
    pub fn kernel(self) -> usize {
        match self {
            Self::Low => 2,
            Self::Medium => 4,
            Self::High => 6,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: Vec3) -> Vec3 {
    let length = dot(a, a).sqrt();
    [a[0] / length, a[1] / length, a[2] / length]
}

pub fn distance_falloff(distance: f64, radius: f64, falloff_pow: f64) -> f64 {
    (1.0 - distance / radius).max(0.0).powf(falloff_pow)
}

pub fn spot_cone(cos_theta: f64, fov: f64, spot_falloff_pow: f64) -> f64 {
    let cos_half = (fov.to_radians() / 2.0).cos();
    let t = ((1.0 - cos_theta) / (1.0 - cos_half)).clamp(0.0, 1.0);
    (1.0 - t.sqrt()).powf(spot_falloff_pow)
}

fn spot_projection(
    location: Vec3,
    rotation: Rotator,
    fov: f64,
    aspect: f64,
    depth_row: [f64; 4],
) -> Mat4 {
    let [forward, right, up] = rotation.axes();
    let row = |axis: Vec3| {
        let along = -(axis[0] * location[0] + axis[1] * location[1] + axis[2] * location[2]);
        [axis[0], axis[1], axis[2], along]
    };
    let view = Mat4::from_rows([row(right), row(up), row(forward), [0.0, 0.0, 0.0, 1.0]]);
    let half = (fov.to_radians() / 2.0).tan();
    let projection = Mat4::from_rows([
        [1.0 / (aspect * half), 0.0, 0.0, 0.0],
        [0.0, 1.0 / half, 0.0, 0.0],
        depth_row,
        [0.0, 0.0, 1.0, 0.0],
    ]);
    projection.mul(&view)
}

pub fn spot_view_projection(
    location: Vec3,
    rotation: Rotator,
    fov: f64,
    aspect: f64,
    near_plane: f64,
) -> Mat4 {
    spot_projection(location, rotation, fov, aspect, [0.0, 0.0, 0.0, near_plane])
}

pub fn spot_shadow_projection(
    location: Vec3,
    rotation: Rotator,
    fov: f64,
    aspect: f64,
    near_plane: f64,
    far_plane: f64,
) -> Mat4 {
    let scale = far_plane / (far_plane - near_plane);
    let depth_row = [0.0, 0.0, scale, -scale * near_plane];
    spot_projection(location, rotation, fov, aspect, depth_row)
}

pub fn shadow_depth(distance: f64, near_plane: f64, far_plane: f64) -> f64 {
    far_plane / (far_plane - near_plane) * (1.0 - near_plane / distance)
}

pub fn shadow_transition(side: u32, near_plane_cm: f64, far_plane_cm: f64) -> f64 {
    let near = near_plane_cm / CM_PER_M;
    let far = far_plane_cm / CM_PER_M;
    (far - near).sqrt() * ((f64::from(side) / 14.4 + 80.0 / 9.0) / near)
}

pub fn gobo_uv(view_projection: &Mat4, point: Vec3) -> Option<[f64; 2]> {
    let clip = view_projection.apply([point[0], point[1], point[2], 1.0]);
    if clip[3] <= 0.0 {
        return None;
    }
    let x = clip[0] / clip[3];
    let y = clip[1] / clip[3];
    if x.abs() > 1.0 || y.abs() > 1.0 {
        return None;
    }
    Some([0.5 + 0.5 * x, 0.5 - 0.5 * y])
}

pub fn gobo_frame(
    anim: GoboAnim,
    frames: usize,
    frame_ticks: u32,
    start_tick: u64,
    tick: u64,
) -> usize {
    if frames <= 1 || frame_ticks == 0 || tick < start_tick {
        return 0;
    }
    let step = (tick - start_tick) / u64::from(frame_ticks);
    let count = frames as u64;
    match anim {
        GoboAnim::None => 0,
        GoboAnim::Loop => (step % count) as usize,
        GoboAnim::Oscillate => {
            let period = 2 * count - 2;
            let phase = step % period;
            if phase < count {
                phase as usize
            } else {
                (period - phase) as usize
            }
        }
    }
}

pub fn specular_power(material_power: f64) -> f64 {
    (10.0 * material_power).exp2() + 1.0
}

pub fn blinn_phong(normal: Vec3, to_light: Vec3, to_camera: Vec3, power: f64) -> f64 {
    let half = normalize([
        to_light[0] + to_camera[0],
        to_light[1] + to_camera[1],
        to_light[2] + to_camera[2],
    ]);
    dot(normal, half).clamp(0.0, 1.0).powf(power)
}

pub fn shadow_receiver(point: Vec3, normal: Vec3, to_light: Vec3, offset_cm: f64) -> Vec3 {
    let along = (1.0 - dot(normal, to_light)) * offset_cm;
    [
        point[0] + normal[0] * along,
        point[1] + normal[1] * along,
        point[2] + normal[2] * along,
    ]
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShadowMap {
    pub side: usize,
    pub stored: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowSpot {
    pub view_projection: Mat4,
    pub location: Vec3,
    pub forward: Vec3,
    pub near_plane: f64,
    pub far_plane: f64,
    pub transition: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterBias {
    pub constant: i32,
    pub slope_scale: f32,
}

impl RasterBias {
    pub fn new(bias_mul: f32, slope_bias_mul: f32) -> Self {
        RasterBias {
            constant: (SHADOW_DEPTH_BIAS * f64::from(bias_mul)).round() as i32,
            slope_scale: (SHADOW_SLOPE_BIAS * f64::from(slope_bias_mul)) as f32,
        }
    }

    pub fn apply(&self, depth: f64, max_depth: f64, max_slope: f64) -> f64 {
        let unit = if max_depth > 0.0 {
            2f64.powi(max_depth.log2().floor() as i32 - DEPTH_MANTISSA_BITS)
        } else {
            0.0
        };
        let biased =
            depth + f64::from(self.constant) * unit + f64::from(self.slope_scale) * max_slope;
        biased.clamp(0.0, 1.0)
    }
}

pub fn shadow_occlusion(stored: f64, depth: f64, transition: f64) -> f64 {
    (1.0 + (stored - depth) * transition).clamp(0.0, 1.0)
}

pub fn shadow_kernel_weight(kernel: usize, index: usize, fraction: f64) -> f64 {
    if index == 0 {
        1.0 - fraction
    } else if index == kernel - 1 {
        fraction
    } else {
        1.0
    }
}

pub fn shadow_factor(
    map: &ShadowMap,
    spot: &ShadowSpot,
    quality: ShadowQuality,
    receiver: Vec3,
) -> f64 {
    let Some(uv) = gobo_uv(&spot.view_projection, receiver) else {
        return 1.0;
    };
    let distance = dot(
        [
            receiver[0] - spot.location[0],
            receiver[1] - spot.location[1],
            receiver[2] - spot.location[2],
        ],
        spot.forward,
    );
    let depth = shadow_depth(distance, spot.near_plane, spot.far_plane);
    let side = map.side as f64;
    let texel = [uv[0] * side - 0.5, uv[1] * side - 0.5];
    let base = texel.map(f64::floor);
    let fraction = [texel[0] - base[0], texel[1] - base[1]];
    let kernel = quality.kernel();
    let first = kernel as i64 / 2 - 1;
    let last = map.side as i64 - 1;
    let mut sum = 0.0;
    for j in 0..kernel {
        let row = (base[1] as i64 - first + j as i64).clamp(0, last) as usize;
        let row_weight = shadow_kernel_weight(kernel, j, fraction[1]);
        for i in 0..kernel {
            let column = (base[0] as i64 - first + i as i64).clamp(0, last) as usize;
            let weight = row_weight * shadow_kernel_weight(kernel, i, fraction[0]);
            let stored = map.stored[row * map.side + column];
            sum += weight * shadow_occlusion(stored, depth, spot.transition);
        }
    }
    if kernel == 2 {
        return sum;
    }
    let spread = (kernel - 1) as f64;
    let amount = sum / (spread * spread);
    amount * amount * (3.0 - 2.0 * amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn distance_falloff_runs_from_one_at_the_light_to_zero_at_the_radius() {
        assert_eq!(distance_falloff(0.0, 500.0, 1.0), 1.0);
        assert_eq!(distance_falloff(500.0, 500.0, 1.0), 0.0);
        assert_eq!(distance_falloff(900.0, 500.0, 1.0), 0.0);
        assert!(close(distance_falloff(125.0, 500.0, 1.0), 0.75));
        assert!(close(distance_falloff(250.0, 500.0, 1.0), 0.5));
    }

    #[test]
    fn distance_falloff_power_raises_the_linear_value() {
        for distance in [0.0, 50.0, 200.0, 499.0] {
            let linear = distance_falloff(distance, 500.0, 1.0);
            assert!(close(
                distance_falloff(distance, 500.0, 2.0),
                linear * linear
            ));
        }
    }

    #[test]
    fn spot_cone_is_one_on_the_axis_and_zero_from_the_edge() {
        let fov = 60.0;
        let edge = (fov / 2.0_f64).to_radians().cos();
        assert_eq!(spot_cone(1.0, fov, 1.0), 1.0);
        assert!(close(spot_cone(edge, fov, 1.0), 0.0));
        assert_eq!(spot_cone(0.0, fov, 1.0), 0.0);
        assert_eq!(spot_cone(-1.0, fov, 1.0), 0.0);
        let inside = 15.0_f64.to_radians().cos();
        let value = spot_cone(inside, fov, 1.0);
        assert!(value > 0.0 && value < 1.0);
    }

    #[test]
    fn spot_falloff_power_raises_the_cone() {
        for degrees in [0.0, 5.0, 12.0, 20.0, 29.0] {
            let cos_theta = f64::to_radians(degrees).cos();
            let one = spot_cone(cos_theta, 60.0, 1.0);
            assert!(close(spot_cone(cos_theta, 60.0, 4.0), one.powi(4)));
        }
    }

    #[test]
    fn spot_cone_shrinks_with_the_angle() {
        let mut previous = spot_cone(1.0, 60.0, 1.0);
        for tenth in 1..=300 {
            let next = spot_cone((f64::from(tenth) / 10.0).to_radians().cos(), 60.0, 1.0);
            assert!(next <= previous, "{tenth}");
            previous = next;
        }
    }

    fn straight_down() -> Mat4 {
        spot_view_projection(
            [0.0, 0.0, 400.0],
            Rotator::new(-90.0, 0.0, 0.0),
            90.0,
            2.0,
            DEFAULT_SPOT_NEAR_PLANE,
        )
    }

    #[test]
    fn the_axis_projects_to_the_gobo_centre() {
        let uv = gobo_uv(&straight_down(), [0.0, 0.0, 0.0]).unwrap();
        assert!(close(uv[0], 0.5) && close(uv[1], 0.5));
        let ahead = spot_view_projection(
            [100.0, 200.0, 50.0],
            Rotator::new(0.0, 0.0, 0.0),
            60.0,
            1.0,
            10.0,
        );
        let uv = gobo_uv(&ahead, [700.0, 200.0, 50.0]).unwrap();
        assert!(close(uv[0], 0.5) && close(uv[1], 0.5));
    }

    #[test]
    fn frustum_corners_project_to_the_gobo_corners() {
        let vp = spot_view_projection(
            [0.0, 0.0, 0.0],
            Rotator::new(0.0, 0.0, 0.0),
            90.0,
            2.0,
            10.0,
        );
        let [forward, right, up] = Rotator::new(0.0, 0.0, 0.0).axes();
        let depth = 300.0;
        let corner = |sx: f64, sy: f64| {
            let x = sx * 2.0 * depth * 0.999_999;
            let y = sy * depth * 0.999_999;
            [
                forward[0] * depth + right[0] * x + up[0] * y,
                forward[1] * depth + right[1] * x + up[1] * y,
                forward[2] * depth + right[2] * x + up[2] * y,
            ]
        };
        let expect = |sx: f64, sy: f64, u: f64, v: f64| {
            let uv = gobo_uv(&vp, corner(sx, sy)).unwrap();
            assert!(
                (uv[0] - u).abs() < 1e-5 && (uv[1] - v).abs() < 1e-5,
                "{uv:?}"
            );
        };
        expect(-1.0, 1.0, 0.0, 0.0);
        expect(1.0, 1.0, 1.0, 0.0);
        expect(-1.0, -1.0, 0.0, 1.0);
        expect(1.0, -1.0, 1.0, 1.0);
    }

    #[test]
    fn aspect_widens_the_frustum_only_across() {
        let vp = straight_down();
        assert!(gobo_uv(&vp, [0.0, -700.0, 0.0]).is_some());
        assert!(gobo_uv(&vp, [700.0, 0.0, 0.0]).is_none());
        assert!(gobo_uv(&vp, [0.0, -900.0, 0.0]).is_none());
    }

    #[test]
    fn points_behind_the_light_are_rejected() {
        let vp = straight_down();
        assert!(gobo_uv(&vp, [0.0, 0.0, 800.0]).is_none());
        assert!(gobo_uv(&vp, [0.0, 0.0, 400.0]).is_none());
    }

    #[test]
    fn none_always_shows_the_first_frame() {
        for tick in 0..100 {
            assert_eq!(gobo_frame(GoboAnim::None, 4, 6, 0, tick), 0);
        }
    }

    #[test]
    fn loop_walks_the_frames_and_wraps() {
        let frames: Vec<usize> = (0..30)
            .step_by(6)
            .map(|tick| gobo_frame(GoboAnim::Loop, 4, 6, 0, tick))
            .collect();
        assert_eq!(frames, vec![0, 1, 2, 3, 0]);
        assert_eq!(gobo_frame(GoboAnim::Loop, 4, 6, 0, 5), 0);
        assert_eq!(gobo_frame(GoboAnim::Loop, 4, 6, 0, 11), 1);
    }

    #[test]
    fn oscillate_goes_there_and_back() {
        let frames: Vec<usize> = (0..9)
            .map(|step| gobo_frame(GoboAnim::Oscillate, 4, 6, 0, step * 6))
            .collect();
        assert_eq!(frames, vec![0, 1, 2, 3, 2, 1, 0, 1, 2]);
        let pair: Vec<usize> = (0..4)
            .map(|step| gobo_frame(GoboAnim::Oscillate, 2, 1, 0, step))
            .collect();
        assert_eq!(pair, vec![0, 1, 0, 1]);
    }

    #[test]
    fn frames_hold_before_the_start_tick_and_count_from_it() {
        assert_eq!(gobo_frame(GoboAnim::Loop, 4, 6, 100, 0), 0);
        assert_eq!(gobo_frame(GoboAnim::Loop, 4, 6, 100, 99), 0);
        assert_eq!(gobo_frame(GoboAnim::Loop, 4, 6, 100, 106), 1);
        assert_eq!(gobo_frame(GoboAnim::Oscillate, 4, 6, 100, 124), 2);
    }

    #[test]
    fn one_frame_gobos_never_change() {
        for anim in [GoboAnim::None, GoboAnim::Loop, GoboAnim::Oscillate] {
            for tick in 0..50 {
                assert_eq!(gobo_frame(anim, 1, 3, 0, tick), 0);
            }
        }
    }

    #[test]
    fn resolutions_and_qualities_follow_hpl2() {
        assert_eq!(ShadowResolution::default(), ShadowResolution::High);
        assert_eq!(ShadowResolution::Low.side(), 256);
        assert_eq!(ShadowResolution::Medium.side(), 512);
        assert_eq!(ShadowResolution::High.side(), 1024);
        assert_eq!(ShadowQuality::default(), ShadowQuality::Medium);
        let all = [
            ShadowQuality::Low,
            ShadowQuality::Medium,
            ShadowQuality::High,
        ];
        for quality in all {
            assert_eq!(ShadowQuality::parse(quality.name()), Some(quality));
        }
        assert_eq!(ShadowQuality::parse("ultra"), None);
        assert_eq!(all.map(ShadowQuality::kernel), [2, 4, 6]);
    }

    #[test]
    fn specular_power_runs_from_two_to_1025() {
        assert_eq!(specular_power(0.0), 2.0);
        assert_eq!(specular_power(1.0), 1025.0);
        assert!(close(specular_power(0.5), 33.0));
    }

    #[test]
    fn blinn_phong_peaks_at_the_mirror_and_narrows_with_power() {
        let up = [0.0, 0.0, 1.0];
        let tilt = |degrees: f64| {
            let r = degrees.to_radians();
            [r.sin(), 0.0, r.cos()]
        };
        assert!(close(blinn_phong(up, tilt(40.0), tilt(-40.0), 33.0), 1.0));
        let mut previous = [1.0, 1.0];
        for step in 1..=20 {
            let off = f64::from(step);
            let low = blinn_phong(up, tilt(40.0), tilt(-40.0 + off), 9.0);
            let high = blinn_phong(up, tilt(40.0), tilt(-40.0 + off), 129.0);
            assert!(low < previous[0] && high < previous[1], "{step}");
            assert!(high < low, "{step}");
            previous = [low, high];
        }
        assert_eq!(
            blinn_phong(up, [1.0, 0.0, -0.2], [-1.0, 0.0, -0.2], 2.0),
            0.0
        );
    }

    #[test]
    fn the_receiver_moves_along_the_normal_by_how_grazing_the_light_is() {
        let up = [0.0, 0.0, 1.0];
        let point = [10.0, 20.0, 0.0];
        assert_eq!(
            shadow_receiver(point, up, up, SHADOW_NORMAL_OFFSET_CM),
            point
        );
        let grazing = shadow_receiver(point, up, [1.0, 0.0, 0.0], SHADOW_NORMAL_OFFSET_CM);
        assert_eq!(grazing, [10.0, 20.0, 0.5]);
        let half = shadow_receiver(point, up, normalize([1.0, 0.0, 1.0]), 0.5);
        assert!(close(half[2], 0.5 * (1.0 - 0.5_f64.sqrt())));
    }

    fn floor_spot(far_plane: f64, side: u32) -> ShadowSpot {
        let rotation = Rotator::new(-90.0, 0.0, 0.0);
        let location = [0.0, 0.0, 400.0];
        ShadowSpot {
            view_projection: spot_view_projection(location, rotation, 90.0, 1.0, 10.0),
            location,
            forward: rotation.axes()[0],
            near_plane: 10.0,
            far_plane,
            transition: shadow_transition(side, 10.0, far_plane),
        }
    }

    fn half_occluded(side: usize, stored: f64) -> ShadowMap {
        let mut map = vec![1.0; side * side];
        for y in 0..side {
            for x in side / 2..side {
                map[y * side + x] = stored;
            }
        }
        ShadowMap { side, stored: map }
    }

    const QUALITIES: [ShadowQuality; 3] = [
        ShadowQuality::Low,
        ShadowQuality::Medium,
        ShadowQuality::High,
    ];

    #[test]
    fn shadow_depth_runs_from_zero_at_the_near_plane_to_one_at_the_far_plane() {
        assert_eq!(shadow_depth(10.0, 10.0, 2000.0), 0.0);
        assert!(close(shadow_depth(2000.0, 10.0, 2000.0), 1.0));
        let mut previous = 0.0;
        for step in 1..100 {
            let depth = shadow_depth(10.0 + f64::from(step) * 19.9, 10.0, 2000.0);
            assert!(depth > previous);
            previous = depth;
        }
    }

    #[test]
    fn the_shadow_projection_keeps_the_spots_xy_and_stores_standard_depth() {
        let rotation = Rotator::new(-60.0, 30.0, 0.0);
        let location = [100.0, -50.0, 300.0];
        let gobo = spot_view_projection(location, rotation, 70.0, 1.5, 10.0);
        let shadow = spot_shadow_projection(location, rotation, 70.0, 1.5, 10.0, 900.0);
        let [forward, right, up] = rotation.axes();
        for (along, across, high) in [(50.0, 10.0, -5.0), (400.0, -80.0, 30.0)] {
            let point: Vec3 = std::array::from_fn(|k| {
                location[k] + forward[k] * along + right[k] * across + up[k] * high
            });
            let a = gobo.apply([point[0], point[1], point[2], 1.0]);
            let b = shadow.apply([point[0], point[1], point[2], 1.0]);
            assert!(close(a[0] / a[3], b[0] / b[3]) && close(a[1] / a[3], b[1] / b[3]));
            assert!(close(b[3], along));
            assert!(close(b[2] / b[3], shadow_depth(along, 10.0, 900.0)));
        }
    }

    #[test]
    fn the_transition_scale_is_hpl3s_in_metres() {
        assert!(close(
            shadow_transition(1024, 10.0, 2000.0),
            19.9_f64.sqrt() * 800.0
        ));
        assert!(close(
            shadow_transition(256, 50.0, 600.0),
            5.5_f64.sqrt() * ((256.0 / 14.4 + 80.0 / 9.0) / 0.5)
        ));
    }

    #[test]
    fn each_kernel_is_a_tent_whose_weights_sum_to_its_span() {
        for kernel in [2, 4, 6] {
            for fraction in [0.0, 0.25, 0.7, 0.999] {
                let sum: f64 = (0..kernel)
                    .map(|i| shadow_kernel_weight(kernel, i, fraction))
                    .sum();
                assert!(close(sum, (kernel - 1) as f64), "{kernel} {fraction}");
            }
        }
        assert_eq!(shadow_kernel_weight(4, 0, 0.25), 0.75);
        assert_eq!(shadow_kernel_weight(4, 2, 0.25), 1.0);
        assert_eq!(shadow_kernel_weight(4, 3, 0.25), 0.25);
    }

    #[test]
    fn the_occlusion_ramp_darkens_over_one_transition_behind_the_stored_depth() {
        assert_eq!(shadow_occlusion(0.5, 0.5, 100.0), 1.0);
        assert_eq!(shadow_occlusion(0.6, 0.5, 100.0), 1.0);
        assert!(close(shadow_occlusion(0.495, 0.5, 100.0), 0.5));
        assert_eq!(shadow_occlusion(0.49, 0.5, 100.0), 0.0);
    }

    #[test]
    fn nothing_in_front_is_lit_and_a_full_occluder_is_dark() {
        let spot = floor_spot(2000.0, 64);
        let open = ShadowMap {
            side: 64,
            stored: vec![1.0; 64 * 64],
        };
        let full = ShadowMap {
            side: 64,
            stored: vec![shadow_depth(200.0, 10.0, 2000.0); 64 * 64],
        };
        for quality in QUALITIES {
            for point in [[0.0, 0.0, 0.0], [150.0, -90.0, 0.0]] {
                assert_eq!(shadow_factor(&open, &spot, quality, point), 1.0);
                assert_eq!(shadow_factor(&full, &spot, quality, point), 0.0);
            }
            for outside in [[0.0, 0.0, 300.0], [0.0, 0.0, 900.0], [900.0, 0.0, 0.0]] {
                assert_eq!(shadow_factor(&full, &spot, quality, outside), 1.0);
            }
        }
    }

    #[test]
    fn low_is_bilinear_across_the_occluders_edge() {
        let spot = floor_spot(2000.0, 64);
        let map = half_occluded(64, shadow_depth(200.0, 10.0, 2000.0));
        let at_edge = shadow_factor(&map, &spot, ShadowQuality::Low, [0.0, 0.0, 0.0]);
        assert!(close(at_edge, 0.5), "{at_edge}");
        let texel = 400.0 * 2.0 / 64.0;
        let right = Rotator::new(-90.0, 0.0, 0.0).axes()[1];
        let across = |cm: f64| right.map(|v| v * cm);
        let quarter = shadow_factor(&map, &spot, ShadowQuality::Low, across(texel / 4.0));
        assert!(close(quarter, 0.25), "{quarter}");
        let far = shadow_factor(&map, &spot, ShadowQuality::Low, across(texel * 2.0));
        assert_eq!(far, 0.0);
    }

    #[test]
    fn medium_and_high_fall_smoothly_and_symmetrically_across_the_edge() {
        let spot = floor_spot(2000.0, 64);
        let map = half_occluded(64, shadow_depth(200.0, 10.0, 2000.0));
        let texel = 400.0 * 2.0 / 64.0;
        let right = Rotator::new(-90.0, 0.0, 0.0).axes()[1];
        let across = |cm: f64| right.map(|v| v * cm);
        for quality in [ShadowQuality::Medium, ShadowQuality::High] {
            let reach = quality.kernel() as f64 / 2.0;
            let mut previous = 1.0;
            for step in -40_i32..=40 {
                let x = f64::from(step) * texel * reach / 40.0;
                let value = shadow_factor(&map, &spot, quality, across(x));
                let mirror = shadow_factor(&map, &spot, quality, across(-x));
                assert!(value <= previous + 1e-12, "{step} {value}");
                assert!(close(value + mirror, 1.0), "{step} {value} {mirror}");
                previous = value;
            }
            assert_eq!(previous, 0.0);
            assert_eq!(
                shadow_factor(&map, &spot, quality, across(-texel * reach)),
                1.0
            );
        }
    }

    #[test]
    fn the_raster_bias_is_hpl3s_half_unit_and_unit_slope_rounded_half_away() {
        let default = RasterBias::new(1.0, 1.0);
        assert_eq!([default.constant, default.slope_scale as i32], [1, 1]);
        assert_eq!(
            RasterBias::new(2.0, 0.5),
            RasterBias {
                constant: 1,
                slope_scale: 0.5
            }
        );
        assert_eq!(RasterBias::new(3.0, 0.0).constant, 2);
        assert_eq!(RasterBias::new(0.0, 0.0).constant, 0);
        assert_eq!(RasterBias::new(0.0, 0.0).slope_scale, 0.0);
    }

    #[test]
    fn the_raster_bias_moves_one_float_step_and_the_slope_and_stays_in_range() {
        let bias = RasterBias::new(1.0, 1.0);
        let depth = shadow_depth(400.0, 10.0, 2000.0);
        assert_eq!(bias.apply(depth, depth, 0.0), depth + 2f64.powi(-24));
        assert!(close(
            bias.apply(depth, depth, 1.0e-4),
            depth + 2f64.powi(-24) + 1.0e-4
        ));
        assert_eq!(bias.apply(0.9999999, 0.9999999, 1.0), 1.0);
        assert_eq!(RasterBias::new(0.0, 0.0).apply(depth, depth, 0.5), depth);
    }
}
