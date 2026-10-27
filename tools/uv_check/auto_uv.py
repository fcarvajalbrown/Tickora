import numpy as np
import trimesh

ZERO_UV_AREA = 1e-12
PROJECTION_AXES = ((2, 1), (0, 2), (0, 1))


def uv_areas(uv, faces):
    corners = uv[faces]
    edge_a = corners[:, 1] - corners[:, 0]
    edge_b = corners[:, 2] - corners[:, 0]
    return np.abs(edge_a[:, 0] * edge_b[:, 1] - edge_a[:, 1] * edge_b[:, 0]) / 2.0


def box_projected(corners, normals, tiles_per_metre):
    dominant = np.argmax(np.abs(normals), axis=1)
    uv = np.empty((len(corners), 3, 2))
    for axis, (u_axis, v_axis) in enumerate(PROJECTION_AXES):
        chosen = dominant == axis
        uv[chosen, :, 0] = corners[chosen, :, u_axis]
        uv[chosen, :, 1] = corners[chosen, :, v_axis]
    return uv * tiles_per_metre


def fill_collapsed_uv(mesh, material, tiles_per_metre=1.0):
    existing = getattr(mesh.visual, "uv", None)
    uv = np.zeros((len(mesh.vertices), 2)) if existing is None or len(existing) != len(mesh.vertices) else np.asarray(existing, dtype=np.float64)
    collapsed = uv_areas(uv, mesh.faces) < ZERO_UV_AREA
    kept_faces = mesh.faces[~collapsed]
    redone = np.nonzero(collapsed)[0]
    corners = mesh.vertices[mesh.faces[redone]]
    new_uv = box_projected(corners, mesh.face_normals[redone], tiles_per_metre)
    base = len(mesh.vertices)
    vertices = np.vstack([mesh.vertices, corners.reshape(-1, 3)])
    uvs = np.vstack([uv, new_uv.reshape(-1, 2)])
    new_faces = base + np.arange(len(redone) * 3).reshape(-1, 3)
    faces = np.empty((len(mesh.faces), 3), dtype=np.int64)
    faces[~collapsed] = kept_faces
    faces[redone] = new_faces
    used = np.unique(faces)
    remap = np.full(len(vertices), -1, dtype=np.int64)
    remap[used] = np.arange(len(used))
    result = trimesh.Trimesh(vertices=vertices[used], faces=remap[faces], process=False)
    result.visual = trimesh.visual.TextureVisuals(uv=uvs[used], material=material)
    result.merge_vertices(merge_tex=False, merge_norm=True)
    return result
