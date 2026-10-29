use crate::camera::Rotator;
use crate::math::{Mat4, Vec3};

pub const DEFAULT_FALLOFF_POW: f64 = 1.0;
pub const DEFAULT_SPOT_FOV: f64 = 60.0;
pub const DEFAULT_SPOT_ASPECT: f64 = 1.0;
pub const DEFAULT_SPOT_NEAR_PLANE: f64 = 10.0;
pub const DEFAULT_SPOT_FALLOFF_POW: f64 = 1.0;
pub const MIN_SPOT_FOV: f64 = 1.0;
pub const MAX_SPOT_FOV: f64 = 179.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GoboAnim {
    #[default]
    None,
    Loop,
    Oscillate,
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
}
