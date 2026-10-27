use std::path::Path;

use gltf::buffer::Source;
use gltf::mesh::Mode;

pub const CENTIMETRES_PER_METRE: f32 = 100.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    pub material: String,
    pub base_color: [f32; 4],
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

impl Mesh {
    pub fn load(path: &Path) -> Result<Mesh, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read mesh {}: {e}", path.display()))?;
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Mesh::from_glb(&name, &bytes).map_err(|e| format!("mesh {}: {e}", path.display()))
    }

    pub fn from_glb(name: &str, bytes: &[u8]) -> Result<Mesh, String> {
        let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| format!("not a readable GLB: {e}"))?;
        if gltf.buffers().len() > 1 {
            return Err("more than one buffer is not supported".to_string());
        }
        if let Some(Source::Uri(uri)) = gltf.buffers().next().map(|buffer| buffer.source()) {
            return Err(format!("external buffer {uri:?} is not supported"));
        }
        let blob = gltf.blob.as_deref();
        let scene = gltf
            .default_scene()
            .or_else(|| gltf.scenes().next())
            .ok_or("the file has no scene")?;
        let mut primitives = Vec::new();
        let mut stack: Vec<(gltf::Node, [[f32; 4]; 4])> =
            scene.nodes().map(|node| (node, IDENTITY)).collect();
        while let Some((node, parent)) = stack.pop() {
            let transform = multiply(&parent, &node.transform().matrix());
            if node.skin().is_some() {
                return Err("skinned meshes are not supported".to_string());
            }
            if let Some(mesh) = node.mesh() {
                for primitive in mesh.primitives() {
                    primitives.push(read_primitive(&primitive, blob, &transform)?);
                }
            }
            stack.extend(node.children().map(|child| (child, transform)));
        }
        primitives.sort_by(|a, b| a.material.cmp(&b.material));
        Ok(Mesh {
            name: name.to_string(),
            primitives,
        })
    }

    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut points = self.primitives.iter().flat_map(|p| p.positions.iter());
        let first = *points.next()?;
        Some(points.fold((first, first), |(low, high), point| {
            (
                [0, 1, 2].map(|i| low[i].min(point[i])),
                [0, 1, 2].map(|i| high[i].max(point[i])),
            )
        }))
    }

    pub fn triangle_count(&self) -> usize {
        self.primitives.iter().map(|p| p.indices.len() / 3).sum()
    }
}

const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn multiply(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for (column, out_column) in out.iter_mut().enumerate() {
        for (row, cell) in out_column.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][row] * b[column][k]).sum();
        }
    }
    out
}

fn transform_point(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|row| m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row])
}

fn transform_normal(m: &[[f32; 4]; 4], n: [f32; 3]) -> [f32; 3] {
    let moved = [0, 1, 2].map(|row| m[0][row] * n[0] + m[1][row] * n[1] + m[2][row] * n[2]);
    let length = moved.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length > 0.0 {
        moved.map(|v| v / length)
    } else {
        moved
    }
}

pub fn gltf_to_unreal(point: [f32; 3]) -> [f32; 3] {
    [point[0], point[2], point[1]].map(|v| v * CENTIMETRES_PER_METRE)
}

fn gltf_direction_to_unreal(direction: [f32; 3]) -> [f32; 3] {
    [direction[0], direction[2], direction[1]]
}

fn read_primitive(
    primitive: &gltf::Primitive,
    blob: Option<&[u8]>,
    transform: &[[f32; 4]; 4],
) -> Result<Primitive, String> {
    let material = primitive.material();
    let label = material.name().unwrap_or("unnamed").to_string();
    if primitive.mode() != Mode::Triangles {
        return Err(format!(
            "primitive {label:?}: mode {:?} is not supported, only triangles",
            primitive.mode()
        ));
    }
    if primitive.morph_targets().next().is_some() {
        return Err(format!(
            "primitive {label:?}: morph targets are not supported"
        ));
    }
    if primitive
        .attributes()
        .any(|(_, accessor)| accessor.sparse().is_some())
    {
        return Err(format!(
            "primitive {label:?}: sparse accessors are not supported"
        ));
    }
    let reader = primitive.reader(|_| blob);
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or_else(|| format!("primitive {label:?} has no POSITION"))?
        .map(|p| gltf_to_unreal(transform_point(transform, p)))
        .collect();
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .ok_or_else(|| format!("primitive {label:?} has no NORMAL"))?
        .map(|n| gltf_direction_to_unreal(transform_normal(transform, n)))
        .collect();
    let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
        Some(coords) => coords.into_f32().collect(),
        None => vec![[0.0, 0.0]; positions.len()],
    };
    let mut indices: Vec<u32> = match reader.read_indices() {
        Some(indices) => indices.into_u32().collect(),
        None => (0..positions.len() as u32).collect(),
    };
    if normals.len() != positions.len() || uvs.len() != positions.len() {
        return Err(format!("primitive {label:?}: attribute counts differ"));
    }
    if !indices.len().is_multiple_of(3) || indices.iter().any(|&i| i as usize >= positions.len()) {
        return Err(format!(
            "primitive {label:?}: indices are not whole triangles"
        ));
    }
    for triangle in indices.chunks_mut(3) {
        triangle.swap(1, 2);
    }
    Ok(Primitive {
        material: label,
        base_color: material.pbr_metallic_roughness().base_color_factor(),
        positions,
        normals,
        uvs,
        indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> Mesh {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/gen/render_fixtures/meshes")
            .join(format!("{name}.glb"));
        Mesh::load(&path).unwrap()
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
    }

    fn primitive<'a>(mesh: &'a Mesh, material: &str) -> &'a Primitive {
        mesh.primitives
            .iter()
            .find(|p| p.material == material)
            .unwrap()
    }

    fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
        let mut low = points[0];
        let mut high = points[0];
        for point in points {
            low = [0, 1, 2].map(|i| low[i].min(point[i]));
            high = [0, 1, 2].map(|i| high[i].max(point[i]));
        }
        (low, high)
    }

    #[test]
    fn meshkit_axes_land_on_unreal_axes_in_centimetres() {
        let marker = fixture("marker");
        assert_eq!(marker.name, "marker");
        assert_eq!(marker.triangle_count(), 24);
        let (low, high) = bounds(&primitive(&marker, "wall").positions);
        assert!(close(low, [-50.0, -25.0, -10.0]), "{low:?}");
        assert!(close(high, [50.0, 25.0, 10.0]), "{high:?}");
        let (low, high) = bounds(&primitive(&marker, "metal").positions);
        let centre = [0, 1, 2].map(|i| (low[i] + high[i]) / 2.0);
        assert!(close(centre, [100.0, -200.0, 300.0]), "{centre:?}");
    }

    #[test]
    fn winding_agrees_with_the_stored_normals() {
        for name in ["marker", "cube", "floor"] {
            for primitive in &fixture(name).primitives {
                for triangle in primitive.indices.chunks(3) {
                    let [a, b, c] = [0, 1, 2].map(|i| primitive.positions[triangle[i] as usize]);
                    let u = [0, 1, 2].map(|i| b[i] - a[i]);
                    let v = [0, 1, 2].map(|i| c[i] - a[i]);
                    let cross = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    let normal = primitive.normals[triangle[0] as usize];
                    let along: f32 = (0..3).map(|i| cross[i] * normal[i]).sum();
                    assert!(
                        along > 0.0,
                        "{name} {}: {cross:?} vs {normal:?}",
                        primitive.material
                    );
                }
            }
        }
    }

    #[test]
    fn reads_material_names_and_colours() {
        let marker = fixture("marker");
        let names: Vec<&str> = marker
            .primitives
            .iter()
            .map(|p| p.material.as_str())
            .collect();
        assert_eq!(names, ["metal", "wall"]);
        let wall = primitive(&marker, "wall");
        let expected = [160.0 / 255.0, 125.0 / 255.0, 88.0 / 255.0, 1.0];
        assert!(
            wall.base_color
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
        assert!(wall.uvs.iter().any(|uv| uv != &[0.0, 0.0]));
    }

    #[test]
    fn cube_sits_on_the_floor_plane() {
        let (low, high) = fixture("cube").bounds().unwrap();
        assert!(close(low, [-50.0, -50.0, 0.0]), "{low:?}");
        assert!(close(high, [50.0, 50.0, 100.0]), "{high:?}");
    }

    #[test]
    fn rejects_files_that_are_not_glb() {
        assert!(Mesh::from_glb("x", b"not a glb").is_err());
        assert!(Mesh::load(Path::new("no/such/file.glb")).is_err());
    }
}
