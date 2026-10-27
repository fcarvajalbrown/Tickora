use std::collections::BTreeMap;
use std::sync::Arc;

use crate::camera::{PerspectiveCamera, Rotator};
use crate::image::Image;
use crate::math::{Mat4, Vec3};
use crate::mesh::Mesh;
use crate::scene::Color;

pub const VERTEX_SIZE: u64 = 48;
pub const MAX_LIGHTS: usize = 16;
pub const CENTIMETRES_PER_METRE: f64 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    pub location: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub attenuation_radius: f64,
}

impl PointLight {
    pub fn falloff(&self, distance: f64) -> f64 {
        let metres = distance / CENTIMETRES_PER_METRE;
        let ratio = distance / self.attenuation_radius;
        let window = (1.0 - ratio.powi(4)).clamp(0.0, 1.0);
        window * window / (metres * metres + 1.0)
    }
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
    pub instances: Vec<MeshInstance>,
    pub lights: Vec<PointLight>,
    pub textures: BTreeMap<String, Arc<Image>>,
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
    pub fn light_bytes(&self) -> Result<Vec<u8>, String> {
        if self.lights.len() > MAX_LIGHTS {
            return Err(format!(
                "{} point lights; at most {MAX_LIGHTS} per frame",
                self.lights.len()
            ));
        }
        for light in &self.lights {
            let numbers = light
                .location
                .iter()
                .copied()
                .chain(light.color.map(f64::from))
                .chain([f64::from(light.intensity), light.attenuation_radius]);
            if numbers.into_iter().any(|v| !v.is_finite()) {
                return Err("point light values must be finite".to_string());
            }
            if light.attenuation_radius <= 0.0 || light.intensity < 0.0 {
                return Err(
                    "point light attenuation_radius must be above 0 and intensity at least 0"
                        .to_string(),
                );
            }
        }
        let mut bytes = Vec::with_capacity(32 + MAX_LIGHTS * 32);
        for value in self.ambient.into_iter().chain([0.0]) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in [self.lights.len() as u32, 0, 0, 0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for index in 0..MAX_LIGHTS {
            let values = match self.lights.get(index) {
                Some(light) => [
                    light.location[0] as f32,
                    light.location[1] as f32,
                    light.location[2] as f32,
                    light.attenuation_radius as f32,
                    light.color[0],
                    light.color[1],
                    light.color[2],
                    light.intensity,
                ],
                None => [0.0; 8],
            };
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
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
            instances,
            lights: Vec::new(),
            textures: BTreeMap::new(),
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
        }
    }

    #[test]
    fn falloff_follows_karis_equation_nine_in_metres() {
        let lamp = light(1000.0);
        assert_eq!(lamp.falloff(0.0), 1.0);
        let at_one_metre = (1.0 - 0.1f64.powi(4)).powi(2) / 2.0;
        assert!((lamp.falloff(100.0) - at_one_metre).abs() < 1e-12);
        let at_half_radius = (1.0 - 0.5f64.powi(4)).powi(2) / (25.0 + 1.0);
        assert!((lamp.falloff(500.0) - at_half_radius).abs() < 1e-12);
        assert_eq!(lamp.falloff(1000.0), 0.0);
        assert_eq!(lamp.falloff(2000.0), 0.0);
    }

    #[test]
    fn light_bytes_pack_ambient_count_and_sixteen_slots() {
        let mut scene = scene(Vec::new());
        scene.ambient = [0.25, 0.5, 1.0];
        scene.lights.push(PointLight {
            location: [1.0, 2.0, 3.0],
            color: [1.0, 0.5, 0.0],
            intensity: 4.0,
            attenuation_radius: 500.0,
        });
        let bytes = scene.light_bytes().unwrap();
        assert_eq!(bytes.len(), 32 + MAX_LIGHTS * 32);
        assert_eq!(floats(&bytes[..16]), [0.25, 0.5, 1.0, 0.0]);
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        assert_eq!(
            floats(&bytes[32..64]),
            [1.0, 2.0, 3.0, 500.0, 1.0, 0.5, 0.0, 4.0]
        );
        assert!(bytes[64..].iter().all(|&b| b == 0));
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
