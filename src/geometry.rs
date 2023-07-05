use std::ops::Index;

use enum_ptr::EnumPtr;
use glam::{BVec3, Vec2, Vec3};

pub mod ray;
pub mod sphere;
pub mod trianglemesh;

use rand::rngs::SmallRng;
pub use ray::Ray;

use crate::{scene::ShapeSample, util::TaggedPtr};

use self::{sphere::Sphere, trianglemesh::Triangle};

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
}

pub struct ShapeHitInfo {
    pub pos: Vec3,
    pub normal: Vec3,
    pub t: f32,
    pub uv: Option<Vec2>,
}

impl ShapeHitInfo {
    pub fn new(pos: Vec3, normal: Vec3, t: f32, uv: Option<Vec2>) -> Self {
        Self { pos, normal, t, uv }
    }
}

#[derive(EnumPtr)]
#[repr(C, usize)]
pub enum Shape {
    Sphere(Box<Sphere>),
    Triangle(Box<Triangle>),
}

impl TaggedPtr<Shape> {
    pub fn intersect(&self, ray: &Ray) -> Option<ShapeHitInfo> {
        self.0.map_ref(|s| match s {
            Shape::Sphere(sphere) => sphere.hit(ray),
            Shape::Triangle(triangle) => triangle.intersect(ray),
        })
    }

    /// Must not be called on non-light Hittables
    pub fn sample_point(&self, rng: &mut SmallRng) -> ShapeSample {
        self.0.map_ref(|s| match s {
            Shape::Sphere(sphere) => sphere.sample_point(rng),
            Shape::Triangle(_) => unreachable!(),
        })
    }

    /// Must not be called on non-light Hittables
    pub fn area(&self) -> f32 {
        self.0.map_ref(|s| match s {
            Shape::Sphere(sphere) => sphere.area(),
            Shape::Triangle(_) => unreachable!(),
        })
    }

    pub fn aabb(&self) -> AABB {
        self.0.map_ref(|s| match s {
            Shape::Sphere(sphere) => sphere.aabb(),
            Shape::Triangle(_) => unreachable!(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(a: Vec3, b: Vec3) -> Self {
        let min = Vec3::min(a, b);
        let max = Vec3::max(a, b);
        Self { min, max }
    }

    /// Taken from PBRTv4.
    /// Original: An Efficient and Robust Ray–Box Intersection Algorithm.
    /// https://people.csail.mit.edu/amy/papers/box-jgt.pdf
    pub fn intersects(
        &self,
        ray: &Ray,
        ray_tmax: f32,
        inv_ray_dir: Vec3,
        dir_is_neg: BVec3,
    ) -> bool {
        let mut tmin = (self[dir_is_neg.x].x - ray.orig.x) * inv_ray_dir.x;
        let mut tmax = (self[!dir_is_neg.x].x - ray.orig.x) * inv_ray_dir.x;
        let tymin = (self[dir_is_neg.y].y - ray.orig.y) * inv_ray_dir.y;
        let tymax = (self[!dir_is_neg.y].y - ray.orig.y) * inv_ray_dir.y;
        // TODO: robust floating-point computation

        if tmin > tymax || tymin > tmax {
            return false;
        }
        if tymin > tmin {
            tmin = tymin;
        }
        if tymax < tmax {
            tmax = tymax;
        }

        let tzmin = (self[dir_is_neg.z].z - ray.orig.z) * inv_ray_dir.z;
        let tzmax = (self[!dir_is_neg.z].z - ray.orig.z) * inv_ray_dir.z;

        if tmin > tzmax || tzmin > tmax {
            return false;
        }
        if tzmin > tmin {
            tmin = tzmin;
        }
        if tzmax < tmax {
            tmax = tzmax;
        }

        return (tmin < ray_tmax) && (tmax > 0.);
    }

    pub fn union_point(self, b: Vec3) -> Self {
        let min = Vec3::min(self.min, b);
        let max = Vec3::max(self.max, b);
        Self { min, max }
    }

    pub fn union_aabb(self, b: AABB) -> Self {
        let min = Vec3::min(self.min, b.min);
        let max = Vec3::max(self.max, b.max);
        Self { min, max }
    }

    pub fn fits_within(&self, other: AABB) -> bool {
        self.min.cmpge(other.min).all() && self.max.cmple(other.max).all()
    }

    pub fn diagonal(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn offset_of(&self, other: Vec3) -> Vec3 {
        let mut off = other - self.min;
        if self.max.x > self.min.x {
            off.x /= self.max.x - self.min.x;
        }
        if self.max.y > self.min.y {
            off.y /= self.max.y - self.min.y;
        }
        if self.max.z > self.min.z {
            off.z /= self.max.z - self.min.z;
        }
        off
    }

    pub fn area(&self) -> f32 {
        let d = self.diagonal();
        2. * (d.x * d.y + d.x * d.z + d.z * d.y)
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) / 2.
    }

    pub fn max_axis(&self) -> Axis {
        let diag = self.diagonal();
        if diag.x > diag.y && diag.x > diag.z {
            Axis::X
        } else if diag.y > diag.z {
            Axis::Y
        } else {
            Axis::Z
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min == self.max
    }

    pub const EMPTY: AABB = AABB {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(f32::MIN),
    };
}

impl Index<bool> for AABB {
    type Output = Vec3;

    fn index(&self, index: bool) -> &Self::Output {
        match index {
            true => &self.max,
            false => &self.min,
        }
    }
}

#[cfg(test)]
mod test_geometry {
    use glam::vec3;

    use super::*;

    #[test]
    fn test_aabb() {
        let aabb_0 = AABB::new(Vec3::ONE, Vec3::NEG_ONE);
        let aabb_1 = AABB::new(Vec3::NEG_ONE, Vec3::ONE);

        assert_eq!(aabb_0, aabb_1);
        assert_eq!(aabb_0.center(), Vec3::ZERO);
        assert_eq!(aabb_1.center(), Vec3::ZERO);

        let aabb_2 = AABB::new(Vec3::ZERO, Vec3::splat(2.));
        assert_eq!(aabb_2.center(), Vec3::ONE);

        let aabb_3 = AABB::new(vec3(-1.8, -0.3, 0.9), vec3(1.2, 1.7, 1.9));
        assert_eq!(aabb_3.area(), 22.);
    }

    #[test]
    fn test_aabb_union() {
        let aabb = AABB::new(Vec3::ZERO, Vec3::ONE);
        let union_point_0 = aabb.union_point(vec3(1.1, 1.2, 1.3));
        let union_point_1 = aabb.union_point(vec3(-0.1, -0.2, -0.3));
        assert_eq!(union_point_0, AABB::new(Vec3::ZERO, vec3(1.1, 1.2, 1.3)));
        assert_eq!(union_point_1, AABB::new(vec3(-0.1, -0.2, -0.3), Vec3::ONE));

        let aabb_intersecting_0 = AABB::new(Vec3::splat(-0.5), Vec3::splat(2.));
        let aabb_intersecting_1 = AABB::new(Vec3::ZERO, Vec3::splat(3.));
        let union_aabb = aabb_intersecting_0.union_aabb(aabb_intersecting_1);
        assert_eq!(union_aabb, AABB::new(Vec3::splat(-0.5), Vec3::splat(3.)));

        let aabb_enclosing_0 = AABB::new(Vec3::splat(-2.), Vec3::splat(2.));
        let aabb_enclosing_1 = AABB::new(Vec3::NEG_ONE, Vec3::ONE);
        let union_aabb = aabb_enclosing_0.union_aabb(aabb_enclosing_1);
        assert_eq!(union_aabb, aabb_enclosing_0);

        let aabb_disjoint_0 = AABB::new(Vec3::splat(-2.), Vec3::NEG_ONE);
        let aabb_disjoint_1 = AABB::new(Vec3::ONE, Vec3::splat(2.));
        let union_aabb = aabb_disjoint_0.union_aabb(aabb_disjoint_1);
        assert_eq!(union_aabb, AABB::new(Vec3::splat(-2.), Vec3::splat(2.)));
    }
}
