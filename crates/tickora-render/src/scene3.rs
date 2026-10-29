use std::collections::BTreeMap;
use std::sync::Arc;

use crate::camera::{PerspectiveCamera, Rotator};
use crate::image::Image;
use crate::light::{self, GoboAnim};
use crate::math::{Mat4, Vec3};
use crate::mesh::Mesh;
use crate::scene::Color;

pub const VERTEX_SIZE: u64 = 48;
pub const MAX_LIGHTS: usize = 16;
pub const MAX_SPOTS: usize = 16;
pub const MAX_GOBO_FRAMES: usize = 64;
pub const MIN_GOBO_SIDE: u32 = 16;
pub const MAX_GOBO_SIDE: u32 = 256;
pub const DEFAULT_SPOT_RADIUS: f64 = 10000.0;
pub const POINT_LIGHT_SIZE: usize = 48;
pub const SPOT_LIGHT_SIZE: usize = 128;
pub const LIGHT_BYTES_SIZE: usize =
    32 + MAX_LIGHTS * POINT_LIGHT_SIZE + MAX_SPOTS * SPOT_LIGHT_SIZE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    pub location: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub attenuation_radius: f64,
    pub falloff_pow: f32,
}

impl PointLight {
    pub fn falloff(&self, distance: f64) -> f64 {
        light::distance_falloff(
            distance,
            self.attenuation_radius,
            f64::from(self.falloff_pow),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GoboRef {
    pub name: String,
    pub anim: GoboAnim,
    pub frame_ticks: u32,
    pub start_tick: u64,
}

impl GoboRef {
    pub fn still(name: &str) -> Self {
        GoboRef {
            name: name.to_string(),
            anim: GoboAnim::None,
            frame_ticks: 1,
            start_tick: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpotLight {
    pub location: Vec3,
    pub rotation: Rotator,
    pub color: [f32; 3],
    pub intensity: f32,
    pub attenuation_radius: f64,
    pub falloff_pow: f32,
    pub fov: f64,
    pub aspect: f64,
    pub near_plane: f64,
    pub spot_falloff_pow: f32,
    pub gobo: Option<GoboRef>,
}

impl SpotLight {
    pub fn new(location: Vec3, rotation: Rotator) -> Self {
        SpotLight {
            location,
            rotation,
            color: [1.0; 3],
            intensity: 1.0,
            attenuation_radius: DEFAULT_SPOT_RADIUS,
            falloff_pow: light::DEFAULT_FALLOFF_POW as f32,
            fov: light::DEFAULT_SPOT_FOV,
            aspect: light::DEFAULT_SPOT_ASPECT,
            near_plane: light::DEFAULT_SPOT_NEAR_PLANE,
            spot_falloff_pow: light::DEFAULT_SPOT_FALLOFF_POW as f32,
            gobo: None,
        }
    }

    pub fn view_projection(&self) -> Mat4 {
        light::spot_view_projection(
            self.location,
            self.rotation,
            self.fov,
            self.aspect,
            self.near_plane,
        )
    }

    fn check(&self) -> Result<(), String> {
        let numbers = self
            .location
            .iter()
            .copied()
            .chain([self.rotation.pitch, self.rotation.yaw, self.rotation.roll])
            .chain(self.color.map(f64::from))
            .chain([f64::from(self.intensity)]);
        if numbers.into_iter().any(|v| !v.is_finite()) {
            return Err("spot light values must be finite".to_string());
        }
        if self.intensity < 0.0 {
            return Err("spot light intensity must be at least 0".to_string());
        }
        let fov_range = light::MIN_SPOT_FOV..=light::MAX_SPOT_FOV;
        if !(self.fov.is_finite() && fov_range.contains(&self.fov)) {
            return Err(format!(
                "spot light fov must be from {} to {} degrees",
                light::MIN_SPOT_FOV,
                light::MAX_SPOT_FOV
            ));
        }
        for (name, value) in [
            ("attenuation_radius", self.attenuation_radius),
            ("falloff_pow", f64::from(self.falloff_pow)),
            ("aspect", self.aspect),
            ("near_plane", self.near_plane),
            ("spot_falloff_pow", f64::from(self.spot_falloff_pow)),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("spot light {name} must be a finite number above 0"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoboLayers<'a> {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<&'a Image>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshInstance {
    pub mesh: Arc<Mesh>,
    pub location: Vec3,
    pub rotation: Rotator,
    pub scale: f64,
    pub color: Option<[f32; 4]>,
}

impl MeshInstance {
    pub fn new(mesh: Arc<Mesh>, location: Vec3) -> Self {
        MeshInstance {
            mesh,
            location,
            rotation: Rotator::default(),
            scale: 1.0,
            color: None,
        }
    }

    pub fn model(&self) -> Mat4 {
        let [forward, right, up] = self.rotation.axes();
        let s = self.scale;
        Mat4::from_rows([
            [forward[0] * s, right[0] * s, up[0] * s, self.location[0]],
            [forward[1] * s, right[1] * s, up[1] * s, self.location[1]],
            [forward[2] * s, right[2] * s, up[2] * s, self.location[2]],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    fn corners(&self) -> Vec<[f64; 4]> {
        let Some((low, high)) = self.mesh.bounds() else {
            return Vec::new();
        };
        let model = self.model();
        (0..8)
            .map(|bits| {
                let pick = |axis: usize| {
                    f64::from(if bits & (1 << axis) == 0 {
                        low[axis]
                    } else {
                        high[axis]
                    })
                };
                model.apply([pick(0), pick(1), pick(2), 1.0])
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene3 {
    pub background: Color,
    pub ambient: [f32; 3],
    pub exposure: f32,
    pub white_point: f32,
    pub instances: Vec<MeshInstance>,
    pub lights: Vec<PointLight>,
    pub spots: Vec<SpotLight>,
    pub textures: BTreeMap<String, Arc<Image>>,
    pub gobos: BTreeMap<String, Vec<Arc<Image>>>,
    pub tick: u64,
    pub camera: PerspectiveCamera,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    pub material: String,
    pub vertices: Vec<u8>,
}

impl Batch {
    pub fn vertex_count(&self) -> u32 {
        (self.vertices.len() as u64 / VERTEX_SIZE) as u32
    }
}

impl Scene3 {
    pub fn gobo_layers(&self) -> Result<GoboLayers<'_>, String> {
        let side_ok =
            |side: u32| side.is_power_of_two() && (MIN_GOBO_SIDE..=MAX_GOBO_SIDE).contains(&side);
        let mut size: Option<(u32, u32)> = None;
        let mut frames = Vec::new();
        for (name, images) in &self.gobos {
            if images.is_empty() {
                return Err(format!("gobo {name:?} has no frames"));
            }
            for image in images {
                let this = (image.width, image.height);
                if !side_ok(this.0) || !side_ok(this.1) {
                    return Err(format!(
                        "gobo {name:?} is {}x{}; each side must be a power of two from {MIN_GOBO_SIDE} to {MAX_GOBO_SIDE}",
                        this.0, this.1
                    ));
                }
                let first = *size.get_or_insert(this);
                if first != this {
                    return Err(format!(
                        "gobo {name:?} is {}x{}; every gobo frame must be {}x{} like the first",
                        this.0, this.1, first.0, first.1
                    ));
                }
                frames.push(image.as_ref());
            }
        }
        if frames.len() > MAX_GOBO_FRAMES {
            return Err(format!(
                "{} gobo frames; at most {MAX_GOBO_FRAMES} per frame",
                frames.len()
            ));
        }
        let (width, height) = size.unwrap_or((0, 0));
        Ok(GoboLayers {
            width,
            height,
            frames,
        })
    }

    pub fn gobo_layer(&self, gobo: &GoboRef) -> Result<usize, String> {
        let mut first = 0;
        for (name, images) in &self.gobos {
            if *name == gobo.name {
                let frame = light::gobo_frame(
                    gobo.anim,
                    images.len(),
                    gobo.frame_ticks,
                    gobo.start_tick,
                    self.tick,
                );
                return Ok(first + frame);
            }
            first += images.len();
        }
        Err(format!("no gobo named {:?}", gobo.name))
    }

    pub fn light_bytes(&self) -> Result<Vec<u8>, String> {
        if self.lights.len() > MAX_LIGHTS {
            return Err(format!(
                "{} point lights; at most {MAX_LIGHTS} per frame",
                self.lights.len()
            ));
        }
        if self.spots.len() > MAX_SPOTS {
            return Err(format!(
                "{} spot lights; at most {MAX_SPOTS} per frame",
                self.spots.len()
            ));
        }
        for light in &self.lights {
            let numbers = light
                .location
                .iter()
                .copied()
                .chain(light.color.map(f64::from))
                .chain([
                    f64::from(light.intensity),
                    light.attenuation_radius,
                    f64::from(light.falloff_pow),
                ]);
            if numbers.into_iter().any(|v| !v.is_finite()) {
                return Err("point light values must be finite".to_string());
            }
            if light.attenuation_radius <= 0.0 || light.intensity < 0.0 {
                return Err(
                    "point light attenuation_radius must be above 0 and intensity at least 0"
                        .to_string(),
                );
            }
            if light.falloff_pow <= 0.0 {
                return Err("point light falloff_pow must be above 0".to_string());
            }
        }
        self.gobo_layers()?;
        let mut layers = Vec::with_capacity(self.spots.len());
        for spot in &self.spots {
            spot.check()?;
            layers.push(match &spot.gobo {
                Some(gobo) => self.gobo_layer(gobo)? as f32,
                None => -1.0,
            });
        }
        let mut bytes = Vec::with_capacity(LIGHT_BYTES_SIZE);
        for value in self.ambient.into_iter().chain([0.0]) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in [self.lights.len() as u32, self.spots.len() as u32, 0, 0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let mut put = |values: &[f32]| {
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        };
        for index in 0..MAX_LIGHTS {
            match self.lights.get(index) {
                Some(light) => put(&[
                    light.location[0] as f32,
                    light.location[1] as f32,
                    light.location[2] as f32,
                    light.attenuation_radius as f32,
                    light.color[0],
                    light.color[1],
                    light.color[2],
                    light.intensity,
                    light.falloff_pow,
                    0.0,
                    0.0,
                    0.0,
                ]),
                None => put(&[0.0; POINT_LIGHT_SIZE / 4]),
            }
        }
        for index in 0..MAX_SPOTS {
            match self.spots.get(index).zip(layers.get(index)) {
                Some((spot, layer)) => {
                    let forward = spot.rotation.axes()[0];
                    let cos_half = (spot.fov.to_radians() / 2.0).cos();
                    put(&spot.view_projection().column_major_f32());
                    put(&[
                        spot.location[0] as f32,
                        spot.location[1] as f32,
                        spot.location[2] as f32,
                        spot.attenuation_radius as f32,
                        spot.color[0],
                        spot.color[1],
                        spot.color[2],
                        spot.intensity,
                        forward[0] as f32,
                        forward[1] as f32,
                        forward[2] as f32,
                        cos_half as f32,
                        spot.falloff_pow,
                        spot.spot_falloff_pow,
                        *layer,
                        0.0,
                    ]);
                }
                None => put(&[0.0; SPOT_LIGHT_SIZE / 4]),
            }
        }
        Ok(bytes)
    }

    pub fn triangles(&self) -> Vec<Batch> {
        self.batches(|indices| indices.to_vec())
    }

    pub fn lines(&self) -> Vec<Batch> {
        self.batches(|indices| {
            let mut edges = std::collections::BTreeSet::new();
            for triangle in indices.chunks(3) {
                for (a, b) in [
                    (triangle[0], triangle[1]),
                    (triangle[1], triangle[2]),
                    (triangle[2], triangle[0]),
                ] {
                    edges.insert((a.min(b), a.max(b)));
                }
            }
            edges.into_iter().flat_map(|(a, b)| [a, b]).collect()
        })
    }

    pub fn off_screen(&self, camera: &PerspectiveCamera, width: u32, height: u32) -> usize {
        let view_projection = camera.view_projection(width, height);
        self.instances
            .iter()
            .filter(|instance| {
                let clips: Vec<[f64; 4]> = instance
                    .corners()
                    .into_iter()
                    .map(|corner| view_projection.apply(corner))
                    .collect();
                let all = |outside: fn(&[f64; 4]) -> bool| clips.iter().all(outside);
                clips.is_empty()
                    || all(|c| c[0] > c[3])
                    || all(|c| c[0] < -c[3])
                    || all(|c| c[1] > c[3])
                    || all(|c| c[1] < -c[3])
                    || all(|c| c[2] > c[3])
            })
            .count()
    }

    fn batches(&self, select: impl Fn(&[u32]) -> Vec<u32>) -> Vec<Batch> {
        let mut by_material: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
        for instance in &self.instances {
            let model = instance.model();
            let rows = model.rows();
            for primitive in &instance.mesh.primitives {
                let color = instance.color.unwrap_or(primitive.base_color);
                let bytes = by_material.entry(&primitive.material).or_default();
                for index in select(&primitive.indices) {
                    let index = index as usize;
                    let p = primitive.positions[index].map(f64::from);
                    let n = primitive.normals[index].map(f64::from);
                    let world = model.apply([p[0], p[1], p[2], 1.0]);
                    let turned = [0, 1, 2]
                        .map(|row| rows[row][0] * n[0] + rows[row][1] * n[1] + rows[row][2] * n[2]);
                    let length = turned.iter().map(|v| v * v).sum::<f64>().sqrt();
                    let normal = turned.map(|v| if length > 0.0 { v / length } else { v });
                    for value in [world[0], world[1], world[2]]
                        .into_iter()
                        .chain(normal)
                        .map(|v| v as f32)
                        .chain(primitive.uvs[index])
                        .chain(color)
                    {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
        by_material
            .into_iter()
            .filter(|(_, vertices)| !vertices.is_empty())
            .map(|(material, vertices)| Batch {
                material: material.to_string(),
                vertices,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Primitive;

    fn quad_mesh(material: &str) -> Arc<Mesh> {
        Arc::new(Mesh {
            name: "quad".to_string(),
            primitives: vec![Primitive {
                material: material.to_string(),
                base_color: [0.5, 0.25, 1.0, 1.0],
                positions: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 10.0, 0.0],
                    [0.0, 10.0, 10.0],
                    [0.0, 0.0, 10.0],
                ],
                normals: vec![[-1.0, 0.0, 0.0]; 4],
                uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                indices: vec![0, 1, 2, 0, 2, 3],
            }],
        })
    }

    fn scene(instances: Vec<MeshInstance>) -> Scene3 {
        Scene3 {
            background: [0, 0, 0, 255],
            ambient: [0.0; 3],
            exposure: 1.0,
            white_point: 11.2,
            instances,
            lights: Vec::new(),
            spots: Vec::new(),
            textures: BTreeMap::new(),
            gobos: BTreeMap::new(),
            tick: 0,
            camera: PerspectiveCamera::new([0.0, 0.0, 0.0], Rotator::default(), 90.0).unwrap(),
        }
    }

    fn floats(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    #[test]
    fn model_places_scales_and_turns_the_mesh() {
        let mut instance = MeshInstance::new(quad_mesh("a"), [100.0, 0.0, 5.0]);
        instance.scale = 2.0;
        instance.rotation = Rotator::new(0.0, 90.0, 0.0);
        let moved = instance.model().apply([10.0, 0.0, 1.0, 1.0]);
        assert!((moved[0] - 100.0).abs() < 1e-9);
        assert!((moved[1] - 20.0).abs() < 1e-9);
        assert!((moved[2] - 7.0).abs() < 1e-9);
    }

    #[test]
    fn triangles_carry_world_position_normal_uv_and_colour() {
        let mut instance = MeshInstance::new(quad_mesh("a"), [100.0, 0.0, 0.0]);
        instance.color = Some([1.0, 0.0, 0.0, 1.0]);
        let batches = scene(vec![instance]).triangles();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].vertex_count(), 6);
        let first = floats(&batches[0].vertices[..VERTEX_SIZE as usize]);
        assert_eq!(
            first,
            [
                100.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0
            ]
        );
    }

    #[test]
    fn batches_group_by_material_in_name_order() {
        let batches = scene(vec![
            MeshInstance::new(quad_mesh("stone"), [100.0, 0.0, 0.0]),
            MeshInstance::new(quad_mesh("dirt"), [200.0, 0.0, 0.0]),
            MeshInstance::new(quad_mesh("stone"), [300.0, 0.0, 0.0]),
        ])
        .triangles();
        let names: Vec<(&str, u32)> = batches
            .iter()
            .map(|b| (b.material.as_str(), b.vertex_count()))
            .collect();
        assert_eq!(names, [("dirt", 6), ("stone", 12)]);
    }

    fn light(distance_radius: f64) -> PointLight {
        PointLight {
            location: [0.0; 3],
            color: [1.0; 3],
            intensity: 1.0,
            attenuation_radius: distance_radius,
            falloff_pow: 1.0,
        }
    }

    fn gobo(width: u32, height: u32) -> Arc<Image> {
        Arc::new(Image::new(width, height, vec![255; (width * height * 4) as usize]).unwrap())
    }

    fn spot() -> SpotLight {
        SpotLight::new([0.0; 3], Rotator::default())
    }

    #[test]
    fn falloff_follows_hpl3_in_centimetres() {
        let mut lamp = light(1000.0);
        assert_eq!(lamp.falloff(0.0), 1.0);
        assert!((lamp.falloff(250.0) - 0.75).abs() < 1e-12);
        assert!((lamp.falloff(500.0) - 0.5).abs() < 1e-12);
        assert_eq!(lamp.falloff(1000.0), 0.0);
        assert_eq!(lamp.falloff(2000.0), 0.0);
        lamp.falloff_pow = 2.0;
        assert!((lamp.falloff(500.0) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn spot_defaults_follow_hpl3() {
        let spot = spot();
        assert_eq!(spot.fov, 60.0);
        assert_eq!(spot.aspect, 1.0);
        assert_eq!(spot.near_plane, 10.0);
        assert_eq!(spot.falloff_pow, 1.0);
        assert_eq!(spot.spot_falloff_pow, 1.0);
        assert_eq!(spot.attenuation_radius, 10000.0);
        assert_eq!(spot.gobo, None);
    }

    #[test]
    fn light_bytes_pack_ambient_counts_lights_and_spots() {
        let mut scene = scene(Vec::new());
        scene.ambient = [0.25, 0.5, 1.0];
        scene.lights.push(PointLight {
            location: [1.0, 2.0, 3.0],
            color: [1.0, 0.5, 0.0],
            intensity: 4.0,
            attenuation_radius: 500.0,
            falloff_pow: 2.0,
        });
        let mut lamp = SpotLight::new([5.0, 6.0, 7.0], Rotator::new(-90.0, 0.0, 0.0));
        lamp.color = [0.0, 1.0, 0.5];
        lamp.intensity = 3.0;
        lamp.attenuation_radius = 800.0;
        lamp.fov = 90.0;
        lamp.spot_falloff_pow = 4.0;
        scene.spots.push(lamp.clone());
        let bytes = scene.light_bytes().unwrap();
        assert_eq!(bytes.len(), LIGHT_BYTES_SIZE);
        assert_eq!(floats(&bytes[..16]), [0.25, 0.5, 1.0, 0.0]);
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        assert_eq!(&bytes[20..24], &1u32.to_le_bytes());
        assert_eq!(
            floats(&bytes[32..80]),
            [1.0, 2.0, 3.0, 500.0, 1.0, 0.5, 0.0, 4.0, 2.0, 0.0, 0.0, 0.0]
        );
        let spots = 32 + MAX_LIGHTS * POINT_LIGHT_SIZE;
        assert!(bytes[80..spots].iter().all(|&b| b == 0));
        assert_eq!(
            floats(&bytes[spots..spots + 64]),
            lamp.view_projection().column_major_f32()
        );
        let values = floats(&bytes[spots + 64..spots + SPOT_LIGHT_SIZE]);
        assert_eq!(values[..8], [5.0, 6.0, 7.0, 800.0, 0.0, 1.0, 0.5, 3.0]);
        assert!(values[8].abs() < 1e-6 && values[9].abs() < 1e-6);
        assert!((values[10] + 1.0).abs() < 1e-6);
        assert!((values[11] - 45f32.to_radians().cos()).abs() < 1e-6);
        assert_eq!(values[12..], [1.0, 4.0, -1.0, 0.0]);
        assert!(bytes[spots + SPOT_LIGHT_SIZE..].iter().all(|&b| b == 0));
    }

    #[test]
    fn light_bytes_reject_too_many_or_bad_lights() {
        let mut scene = scene(Vec::new());
        scene.lights = vec![light(100.0); MAX_LIGHTS + 1];
        assert!(scene.light_bytes().is_err());
        scene.lights = vec![light(0.0)];
        assert!(scene.light_bytes().is_err());
        scene.lights = vec![light(f64::NAN)];
        assert!(scene.light_bytes().is_err());
        let mut flat = light(100.0);
        flat.falloff_pow = 0.0;
        scene.lights = vec![flat];
        assert!(scene.light_bytes().is_err());
        flat.falloff_pow = f32::INFINITY;
        scene.lights = vec![flat];
        assert!(scene.light_bytes().is_err());
    }

    #[test]
    fn light_bytes_reject_bad_spots() {
        let mut scene = scene(Vec::new());
        scene.spots = vec![spot(); MAX_SPOTS + 1];
        assert!(scene.light_bytes().is_err());
        let bad: [fn(&mut SpotLight); 13] = [
            |s| s.fov = 0.5,
            |s| s.fov = 179.5,
            |s| s.fov = f64::NAN,
            |s| s.aspect = 0.0,
            |s| s.aspect = f64::INFINITY,
            |s| s.near_plane = -1.0,
            |s| s.attenuation_radius = 0.0,
            |s| s.falloff_pow = 0.0,
            |s| s.spot_falloff_pow = f32::NAN,
            |s| s.intensity = -1.0,
            |s| s.location[0] = f64::NAN,
            |s| s.rotation.yaw = f64::INFINITY,
            |s| s.gobo = Some(GoboRef::still("missing")),
        ];
        for (index, spoil) in bad.iter().enumerate() {
            let mut lamp = spot();
            spoil(&mut lamp);
            scene.spots = vec![lamp];
            assert!(scene.light_bytes().is_err(), "case {index}");
        }
        for fov in [1.0, 179.0] {
            let mut lamp = spot();
            lamp.fov = fov;
            scene.spots = vec![lamp];
            assert!(scene.light_bytes().is_ok(), "fov {fov}");
        }
    }

    #[test]
    fn gobo_layers_follow_name_order_and_the_tick() {
        let mut scene = scene(Vec::new());
        scene
            .gobos
            .insert("flicker".to_string(), vec![gobo(16, 16); 4]);
        scene.gobos.insert("bars".to_string(), vec![gobo(16, 16)]);
        let layers = scene.gobo_layers().unwrap();
        assert_eq!(
            (layers.width, layers.height, layers.frames.len()),
            (16, 16, 5)
        );
        let flicker = GoboRef {
            name: "flicker".to_string(),
            anim: GoboAnim::Loop,
            frame_ticks: 6,
            start_tick: 0,
        };
        assert_eq!(scene.gobo_layer(&GoboRef::still("bars")), Ok(0));
        assert_eq!(scene.gobo_layer(&flicker), Ok(1));
        scene.tick = 13;
        assert_eq!(scene.gobo_layer(&flicker), Ok(3));
        let mut lamp = spot();
        lamp.gobo = Some(flicker);
        scene.spots.push(lamp);
        let bytes = scene.light_bytes().unwrap();
        let at = 32 + MAX_LIGHTS * POINT_LIGHT_SIZE + 64 + 14 * 4;
        assert_eq!(floats(&bytes[at..at + 4]), [3.0]);
    }

    #[test]
    fn gobo_layers_reject_bad_sizes_counts_and_empty_gobos() {
        let mut scene = scene(Vec::new());
        assert_eq!(scene.gobo_layers().unwrap().frames.len(), 0);
        for (width, height) in [(8, 16), (16, 512), (24, 32)] {
            scene.gobos = BTreeMap::from([("g".to_string(), vec![gobo(width, height)])]);
            assert!(scene.gobo_layers().is_err(), "{width}x{height}");
        }
        scene.gobos = BTreeMap::from([("g".to_string(), vec![gobo(32, 16)])]);
        assert!(scene.gobo_layers().is_ok());
        scene.gobos = BTreeMap::from([
            ("a".to_string(), vec![gobo(16, 16)]),
            ("b".to_string(), vec![gobo(32, 32)]),
        ]);
        assert!(scene.gobo_layers().is_err());
        scene.gobos = BTreeMap::from([("g".to_string(), Vec::new())]);
        assert!(scene.gobo_layers().is_err());
        scene.gobos = BTreeMap::from([("g".to_string(), vec![gobo(16, 16); MAX_GOBO_FRAMES])]);
        assert!(scene.gobo_layers().is_ok());
        scene.gobos = BTreeMap::from([("g".to_string(), vec![gobo(16, 16); MAX_GOBO_FRAMES + 1])]);
        assert!(scene.gobo_layers().is_err());
        assert!(scene.light_bytes().is_err());
    }

    #[test]
    fn lines_list_each_edge_once() {
        let batches = scene(vec![MeshInstance::new(quad_mesh("a"), [100.0, 0.0, 0.0])]).lines();
        assert_eq!(batches[0].vertex_count(), 10);
    }

    #[test]
    fn counts_instances_outside_the_frustum() {
        let scene = scene(vec![
            MeshInstance::new(quad_mesh("a"), [100.0, -5.0, -5.0]),
            MeshInstance::new(quad_mesh("a"), [-100.0, 0.0, 0.0]),
            MeshInstance::new(quad_mesh("a"), [100.0, 500.0, 0.0]),
            MeshInstance::new(quad_mesh("a"), [5.0, -5.0, -5.0]),
        ]);
        assert_eq!(scene.off_screen(&scene.camera, 64, 64), 3);
    }
}
