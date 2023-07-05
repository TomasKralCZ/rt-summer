use crate::{
    geometry::Ray,
    math::barycentric_interp,
    pbrt_loader::scene_description::{Material, TriMesh},
    sampling::sample_uniform_triangle,
    scene::ShapeSample,
};

use glam::{Vec2, Vec3};
use rand::rngs::SmallRng;
use std::sync::Arc;

use super::{ShapeHitInfo, AABB};

pub struct TriHitInfo {
    pos: Vec3,
    t: f32,
    bar: Vec3,
}

pub struct TriangleMesh {
    material: Arc<Material>,
    reverse_normals: bool,

    indices: Box<[i32]>,
    pos: Box<[Vec3]>,
    normals: Option<Box<[Vec3]>>,
    uvs: Option<Box<[Vec2]>>,
    tangents: Option<Box<[Vec3]>>,
}

impl TriangleMesh {
    pub fn new(mesh: TriMesh, material: Arc<Material>, reverse_normals: bool) -> Self {
        let TriMesh {
            indices,
            pos,
            normals,
            tangents,
            uvs,
        } = mesh;

        // TODO: this should be in the loading code, don't know if it is
        debug_assert!(indices.len() % 3 == 0);

        Self {
            material,
            reverse_normals,
            indices: indices.into_boxed_slice(),
            pos: pos.into_boxed_slice(),
            normals: normals.map(|n| n.into_boxed_slice()),
            uvs: uvs.map(|uv| uv.into_boxed_slice()),
            tangents: tangents.map(|t| t.into_boxed_slice()),
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn material(&self) -> Arc<Material> {
        self.material.clone()
    }
}

pub struct Triangle {
    /// TODO: custom allocator for Arc https://github.com/rust-lang/rust/pull/89132
    mesh: Arc<TriangleMesh>,
    /// Triangle index in the TriMesh
    id: u64,
}

impl Triangle {
    pub fn new(mesh: Arc<TriangleMesh>, id: u64) -> Self {
        Self { mesh, id }
    }

    fn get_indices(&self) -> (usize, usize, usize) {
        let i0 = self.mesh.indices[self.id as usize * 3];
        let i1 = self.mesh.indices[self.id as usize * 3 + 1];
        let i2 = self.mesh.indices[self.id as usize * 3 + 2];
        (i0 as usize, i1 as usize, i2 as usize)
    }

    fn get_positions(&self) -> (Vec3, Vec3, Vec3) {
        let (i0, i1, i2) = self.get_indices();

        let p0 = self.mesh.pos[i0];
        let p1 = self.mesh.pos[i1];
        let p2 = self.mesh.pos[i2];

        (p0, p1, p2)
    }

    /// Möller-Trumbore algorithm
    pub fn intersect(&self, ray: &Ray) -> Option<ShapeHitInfo> {
        let eps = 0.0000001;

        let (p0, p1, p2) = self.get_positions();

        let e1 = p1 - p0;
        let e2 = p2 - p0;

        let h = ray.dir.cross(e2);
        let a = e1.dot(h);

        if a > -eps && a < eps {
            return None;
        }

        let f = 1. / a;
        let s = ray.orig - p0;
        let u = f * s.dot(h);
        if u < 0. || u > 1. {
            return None;
        }

        let q = s.cross(e1);
        let v = f * ray.dir.dot(q);
        if v < 0. || u + v > 1. {
            return None;
        }

        let t = f * e2.dot(q);
        if t > eps {
            let pos = ray.orig + ray.dir * t;

            // barycentric coords
            let r = 1. - u - v;

            let bar = [r, u, v];
            let (i0, i1, i2) = self.get_indices();

            let uv = self
                .mesh
                .uvs
                .as_ref()
                .map(|uvs| barycentric_interp(&bar, &uvs[i0], &uvs[i1], &uvs[i2]));

            let normal = self.get_normal(bar, (p0, p1, p2), (i0, i1, i2));
            return Some(ShapeHitInfo::new(pos, normal, t, uv));
        }

        None
    }

    pub fn get_normal(
        &self,
        bar: [f32; 3],
        (p0, p1, p2): (Vec3, Vec3, Vec3),
        (i0, i1, i2): (usize, usize, usize),
    ) -> Vec3 {
        let normal = if let Some(n) = &self.mesh.normals {
            barycentric_interp(&bar, &n[i0], &n[i1], &n[i2])
        } else {
            let v0 = p1 - p0;
            let v1 = p2 - p0;
            v0.cross(v1)
        };

        let normal = if self.mesh.reverse_normals {
            -normal
        } else {
            normal
        };

        normal.normalize()
    }

    pub fn sample_point(&self, rng: &mut SmallRng) -> ShapeSample {
        let bar = sample_uniform_triangle(rng);

        let (p0, p1, p2) = self.get_positions();
        let (i0, i1, i2) = self.get_indices();
        let pos = barycentric_interp(&bar, &p0, &p1, &p2);
        let normal = self.get_normal(bar, (p0, p1, p2), (i0, i1, i2));

        ShapeSample::new(pos, normal)
    }

    pub fn area(&self) -> f32 {
        let (p0, p1, p2) = self.get_positions();
        let v0 = p1 - p0;
        let v1 = p2 - p0;
        v0.cross(v1).length() / 2.
    }

    pub fn mesh(&self) -> &TriangleMesh {
        self.mesh.as_ref()
    }

    pub fn aabb(&self) -> AABB {
        let (p0, p1, p2) = self.get_positions();

        let aabb = AABB::new(p0, p1);
        let aabb = aabb.union_point(p2);
        aabb
    }
}
