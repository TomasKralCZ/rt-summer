use std::sync::Arc;

use enum_ptr::EnumPtr;
use rand::rngs::SmallRng;

use crate::{
    geometry::{trianglemesh::Triangle, Ray, Shape, AABB},
    pbrt_loader::scene_description::Material,
    util::TaggedPtr,
};

use super::{HitInfo, LightId, ShapeSample};

pub struct MeshTrianglePrimitive {
    triangle: Triangle,
}

impl MeshTrianglePrimitive {
    pub fn new(triangle: Triangle) -> Self {
        Self { triangle }
    }
}

pub struct MeshTriangleLightPrimitive {
    triangle: Triangle,
    light: LightId,
}

impl MeshTriangleLightPrimitive {
    pub fn new(triangle: Triangle, light: LightId) -> Self {
        Self { triangle, light }
    }

    pub fn light(&self) -> usize {
        self.light
    }
}

pub struct SimplePrimtive {
    shape: TaggedPtr<Shape>,
    material: Arc<Material>,
}

impl SimplePrimtive {
    pub fn new(shape: TaggedPtr<Shape>, material: Arc<Material>) -> Self {
        Self { shape, material }
    }
}

pub struct LightPrimitive {
    shape: TaggedPtr<Shape>,
    material: Arc<Material>,
    light: LightId,
}

impl LightPrimitive {
    pub fn new(shape: TaggedPtr<Shape>, material: Arc<Material>, light: LightId) -> Self {
        Self {
            shape,
            material,
            light,
        }
    }

    pub fn light(&self) -> usize {
        self.light
    }
}

#[derive(EnumPtr)]
#[repr(C, usize)]
pub enum Primitive {
    // Mesh triangles don't need to store the material on their own
    MeshTriangle(Box<MeshTrianglePrimitive>),
    MeshTriangleLight(Box<MeshTriangleLightPrimitive>),
    Simple(Box<SimplePrimtive>),
    Light(Box<LightPrimitive>),
}

impl TaggedPtr<Primitive> {
    pub fn intersect(&self, ray: &Ray) -> Option<HitInfo> {
        self.0.map_ref(|p| match p {
            Primitive::MeshTriangle(triangle) => {
                let shape_hitinfo = triangle.triangle.intersect(ray);
                shape_hitinfo.map(|sh| {
                    HitInfo::from_shape_hitinfo(sh, triangle.triangle.mesh().material(), None)
                })
            }
            Primitive::MeshTriangleLight(light_triangle) => {
                let shape_hitinfo = light_triangle.triangle.intersect(ray);
                shape_hitinfo.map(|sh| {
                    HitInfo::from_shape_hitinfo(
                        sh,
                        light_triangle.triangle.mesh().material(),
                        Some(light_triangle.light),
                    )
                })
            }
            Primitive::Simple(primitive) => {
                let shape_hitinfo = primitive.shape.intersect(ray);
                shape_hitinfo.map(|sh| {
                    HitInfo::from_shape_hitinfo(sh, Arc::clone(&primitive.material), None)
                })
            }
            Primitive::Light(light_primitive) => {
                let shape_hitinfo = light_primitive.shape.intersect(ray);
                shape_hitinfo.map(|sh| {
                    HitInfo::from_shape_hitinfo(
                        sh,
                        Arc::clone(&light_primitive.material),
                        Some(light_primitive.light),
                    )
                })
            }
        })
    }

    /// Should not need to be called on non-light Hittables
    pub fn sample_point(&self, rng: &mut SmallRng) -> ShapeSample {
        self.0.map_ref(|p| match p {
            Primitive::MeshTriangle(_) => unreachable!(),
            Primitive::MeshTriangleLight(light_triangle) => {
                light_triangle.triangle.sample_point(rng)
            }
            Primitive::Simple(_) => unreachable!(),
            Primitive::Light(light_primitive) => light_primitive.shape.sample_point(rng),
        })
    }

    pub fn area(&self) -> f32 {
        self.0.map_ref(|p| match p {
            Primitive::MeshTriangle(triangle) => triangle.triangle.area(),
            Primitive::MeshTriangleLight(light_triangle) => light_triangle.triangle.area(),
            Primitive::Simple(primitive) => primitive.shape.area(),
            Primitive::Light(light_primitive) => light_primitive.shape.area(),
        })
    }

    pub fn aabb(&self) -> AABB {
        self.0.map_ref(|p| match p {
            Primitive::MeshTriangle(triangle) => triangle.triangle.aabb(),
            Primitive::MeshTriangleLight(light_triangle) => light_triangle.triangle.aabb(),
            Primitive::Simple(primitive) => primitive.shape.aabb(),
            Primitive::Light(primitive_light) => primitive_light.shape.aabb(),
        })
    }
}
