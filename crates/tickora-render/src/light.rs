use crate::camera::Rotator;
use crate::math::{Mat4, Vec3};
use tickora_core::rng::Rng;

pub const DEFAULT_FALLOFF_POW: f64 = 1.0;
pub const DEFAULT_SPOT_FOV: f64 = 60.0;
pub const DEFAULT_SPOT_ASPECT: f64 = 1.0;
pub const DEFAULT_SPOT_NEAR_PLANE: f64 = 10.0;
pub const DEFAULT_SPOT_FALLOFF_POW: f64 = 1.0;
pub const MIN_SPOT_FOV: f64 = 1.0;
pub const MAX_SPOT_FOV: f64 = 179.0;
pub const SHADOW_NORMAL_OFFSET_CM: f64 = 3.0;
pub const JITTER_TILE: usize = 32;
pub const JITTER_SEED: u64 = 0;

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
    pub fn taps(self) -> usize {
        match self {
            Self::Low => 1,
            Self::Medium => 16,
            Self::High => 32,
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

pub fn spot_view_projection(
    location: Vec3,
    rotation: Rotator,
    fov: f64,
    aspect: f64,
    near_plane: f64,
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
        [0.0, 0.0, 0.0, near_plane],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    projection.mul(&view)
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JitterMap {
    pub samples: usize,
    pub texels: Vec<[u8; 4]>,
}

impl JitterMap {
    pub fn width(&self) -> usize {
        JITTER_TILE
    }

    pub fn height(&self) -> usize {
        JITTER_TILE * self.samples / 2
    }

    pub fn offset(&self, pixel: [u32; 2], tap: usize) -> [f64; 2] {
        let x = pixel[0] as usize % JITTER_TILE;
        let y = pixel[1] as usize % JITTER_TILE + JITTER_TILE * (tap / 2);
        let texel = self.texels[y * JITTER_TILE + x];
        let pair = 2 * (tap % 2);
        let decode = |byte: u8| f64::from(byte) / 255.0 * 2.0 - 1.0;
        [decode(texel[pair]), decode(texel[pair + 1])]
    }
}

pub fn jitter_map(samples: usize) -> JitterMap {
    assert!(
        samples >= 4 && samples.is_power_of_two(),
        "jitter samples must be a power of two from 4"
    );
    let mut rng = Rng::from_seed(JITTER_SEED);
    let grid = ((samples as f64).sqrt() + 0.5) as usize;
    let cells = grid * grid;
    let tile = JITTER_TILE * JITTER_TILE;
    let mut texels = vec![[0u8; 4]; tile * samples / 2];
    let mut unit = || f64::from(rng.next_u32()) / 4_294_967_296.0 - 0.5;
    for texel in 0..tile {
        let mut cell = Vec::with_capacity(cells);
        for y in 0..grid {
            for x in 0..grid {
                let jitter_x = unit();
                let jitter_y = unit();
                cell.push([
                    (x as f64 + 0.5 + jitter_x) / grid as f64,
                    (y as f64 + 0.5 + jitter_y) / grid as f64,
                ]);
            }
        }
        let mut offsets: Vec<[f64; 2]> = (0..samples)
            .map(|i| {
                let [u, v] = cell[i * cells / samples];
                let angle = std::f64::consts::TAU * u;
                [v.sqrt() * angle.cos(), v.sqrt() * angle.sin()]
            })
            .collect();
        let length = |o: &[f64; 2]| o[0] * o[0] + o[1] * o[1];
        offsets.sort_by(|a, b| length(b).total_cmp(&length(a)));
        let encode = |value: f64| ((value + 1.0) * 0.5 * 255.0) as u8;
        for slice in 0..samples / 2 {
            let [a, b] = [offsets[2 * slice], offsets[2 * slice + 1]];
            texels[slice * tile + texel] = [encode(a[0]), encode(a[1]), encode(b[0]), encode(b[1])];
        }
    }
    JitterMap { samples, texels }
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
    pub bias_cm: f64,
    pub blur: f64,
}

pub fn shadow_factor(
    map: &ShadowMap,
    spot: &ShadowSpot,
    jitter: Option<&JitterMap>,
    receiver: Vec3,
    pixel: [u32; 2],
) -> f64 {
    let Some(uv) = gobo_uv(&spot.view_projection, receiver) else {
        return 1.0;
    };
    let depth = dot(
        [
            receiver[0] - spot.location[0],
            receiver[1] - spot.location[1],
            receiver[2] - spot.location[2],
        ],
        spot.forward,
    ) - spot.bias_cm;
    let last = (map.side - 1) as f64;
    let tap = |offset: [f64; 2]| {
        let x = (uv[0] * map.side as f64 + offset[0] * spot.blur)
            .floor()
            .clamp(0.0, last);
        let y = (uv[1] * map.side as f64 + offset[1] * spot.blur)
            .floor()
            .clamp(0.0, last);
        let stored = map.stored[y as usize * map.side + x as usize];
        if stored * depth <= spot.near_plane {
            1.0
        } else {
            0.0
        }
    };
    let Some(jitter) = jitter else {
        return tap([0.0, 0.0]);
    };
    let taps = jitter.samples as f64;
    let first: f64 = (0..4).map(|i| tap(jitter.offset(pixel, i))).sum();
    if first == 0.0 || first == 4.0 {
        return first / 4.0;
    }
    let rest: f64 = (4..jitter.samples)
        .map(|i| tap(jitter.offset(pixel, i)))
        .sum();
    first / taps + rest / taps
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
        assert_eq!(all.map(ShadowQuality::taps), [1, 16, 32]);
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
        assert_eq!(grazing, [10.0, 20.0, 3.0]);
        let half = shadow_receiver(point, up, normalize([1.0, 0.0, 1.0]), 3.0);
        assert!(close(half[2], 3.0 * (1.0 - 0.5_f64.sqrt())));
    }

    fn jitter_hash(map: &JitterMap) -> u64 {
        let mut hasher = tickora_core::hash::StateHasher::new();
        for texel in &map.texels {
            hasher.write_bytes(texel);
        }
        hasher.finish()
    }

    #[test]
    fn jitter_maps_are_byte_stable_and_sorted_inside_the_disk() {
        for samples in [16, 32] {
            let map = jitter_map(samples);
            assert_eq!(map, jitter_map(samples));
            assert_eq!(map.texels.len(), map.width() * map.height());
            assert_eq!(map.height(), JITTER_TILE * samples / 2);
            for y in 0..JITTER_TILE as u32 {
                for x in 0..JITTER_TILE as u32 {
                    let lengths: Vec<f64> = (0..samples)
                        .map(|tap| {
                            let [ox, oy] = map.offset([x, y], tap);
                            (ox * ox + oy * oy).sqrt()
                        })
                        .collect();
                    for length in &lengths {
                        assert!(*length <= 1.0 + 3.0 / 255.0, "{length}");
                    }
                    for pair in lengths.windows(2) {
                        assert!(pair[0] + 6.0 / 255.0 >= pair[1], "{pair:?}");
                    }
                }
            }
            assert_eq!(map.offset([3, 5], 7), map.offset([35, 69], 7));
        }
        assert_eq!(
            [jitter_hash(&jitter_map(16)), jitter_hash(&jitter_map(32))],
            [3694456948837816401, 18414989285065015924]
        );
    }

    fn floor_spot(blur: f64) -> ShadowSpot {
        let rotation = Rotator::new(-90.0, 0.0, 0.0);
        let location = [0.0, 0.0, 400.0];
        ShadowSpot {
            view_projection: spot_view_projection(location, rotation, 90.0, 1.0, 10.0),
            location,
            forward: rotation.axes()[0],
            near_plane: 10.0,
            bias_cm: 1.0,
            blur,
        }
    }

    fn half_occluded(side: usize, occluder_depth: f64) -> ShadowMap {
        let mut stored = vec![0.0; side * side];
        for y in 0..side {
            for x in side / 2..side {
                stored[y * side + x] = 10.0 / occluder_depth;
            }
        }
        ShadowMap { side, stored }
    }

    #[test]
    fn nothing_in_front_is_lit_and_a_full_occluder_is_dark() {
        let spot = floor_spot(2.0);
        let open = ShadowMap {
            side: 64,
            stored: vec![0.0; 64 * 64],
        };
        let full = ShadowMap {
            side: 64,
            stored: vec![10.0 / 200.0; 64 * 64],
        };
        let jitter = jitter_map(16);
        for point in [[0.0, 0.0, 0.0], [150.0, -90.0, 0.0]] {
            assert_eq!(
                shadow_factor(&open, &spot, Some(&jitter), point, [4, 9]),
                1.0
            );
            assert_eq!(
                shadow_factor(&full, &spot, Some(&jitter), point, [4, 9]),
                0.0
            );
            assert_eq!(shadow_factor(&full, &spot, None, point, [4, 9]), 0.0);
        }
        assert_eq!(
            shadow_factor(&full, &spot, None, [0.0, 0.0, 300.0], [0, 0]),
            1.0
        );
        assert_eq!(
            shadow_factor(&full, &spot, None, [0.0, 0.0, 900.0], [0, 0]),
            1.0
        );
        assert_eq!(
            shadow_factor(&full, &spot, None, [900.0, 0.0, 0.0], [0, 0]),
            1.0
        );
    }

    #[test]
    fn the_bias_lets_a_surface_light_itself() {
        let spot = floor_spot(0.0);
        let own = ShadowMap {
            side: 8,
            stored: vec![10.0 / 400.0; 64],
        };
        assert_eq!(
            shadow_factor(&own, &spot, None, [0.0, 0.0, 0.0], [0, 0]),
            1.0
        );
        let unbiased = ShadowSpot {
            bias_cm: 0.0,
            ..spot
        };
        let ahead = ShadowMap {
            side: 8,
            stored: vec![10.0 / 399.5; 64],
        };
        assert_eq!(
            shadow_factor(&ahead, &unbiased, None, [0.0, 0.0, 0.0], [0, 0]),
            0.0
        );
        assert_eq!(
            shadow_factor(&ahead, &spot, None, [0.0, 0.0, 0.0], [0, 0]),
            1.0
        );
    }

    #[test]
    fn low_takes_one_tap_at_the_projected_texel() {
        let spot = floor_spot(8.0);
        let map = half_occluded(64, 200.0);
        let occluded = shadow_factor(&map, &spot, None, [0.0, 3.0, 0.0], [0, 0]);
        let open = shadow_factor(&map, &spot, None, [0.0, -3.0, 0.0], [0, 0]);
        assert_eq!([occluded, open], [0.0, 1.0]);
    }

    #[test]
    fn the_four_tap_early_out_is_the_mean_of_every_tap_on_a_penumbra() {
        let map = half_occluded(64, 200.0);
        let spot = floor_spot(6.0);
        for samples in [16, 32] {
            let jitter = jitter_map(samples);
            let mut penumbra = 0;
            for step in -40_i32..=40 {
                let point = [0.0, f64::from(step) * 0.5, 0.0];
                let uv = gobo_uv(&spot.view_projection, point).unwrap();
                for pixel in [[0, 0], [7, 30], [31, 31], [12, 3]] {
                    let tap = |i: usize| {
                        let offset = jitter.offset(pixel, i);
                        let x = (uv[0] * 64.0 + offset[0] * spot.blur)
                            .floor()
                            .clamp(0.0, 63.0);
                        let y = (uv[1] * 64.0 + offset[1] * spot.blur)
                            .floor()
                            .clamp(0.0, 63.0);
                        let stored = map.stored[y as usize * 64 + x as usize];
                        if stored * (400.0 - spot.bias_cm) <= 10.0 {
                            1.0
                        } else {
                            0.0
                        }
                    };
                    let first: f64 = (0..4).map(tap).sum();
                    let mean = (0..samples).map(tap).sum::<f64>() / samples as f64;
                    let value = shadow_factor(&map, &spot, Some(&jitter), point, pixel);
                    if first == 0.0 || first == 4.0 {
                        assert_eq!(value, first / 4.0);
                    } else {
                        penumbra += 1;
                        assert!(close(value, mean), "{value} {mean}");
                    }
                    if step.abs() > 20 {
                        assert!(close(value, mean), "{step} {value} {mean}");
                    }
                }
            }
            assert!(penumbra > 10, "{penumbra}");
        }
    }
}
