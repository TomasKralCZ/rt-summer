use std::f32::consts::PI;

use glam::Vec3;
use rand::rngs::SmallRng;

use crate::{
    geometry::Ray,
    math::sqr,
    pbrt_loader::scene_description::{self, ShapeWithParams},
    sampling,
    scene::ShapeSample,
};

use super::{ShapeHitInfo, AABB};

pub struct Sphere {
    center: Vec3,
    radius: f32,
    area: f32,

    bh_index: usize,
}

impl Sphere {
    pub fn new(shape: &ShapeWithParams, sphere: &scene_description::Sphere) -> Self {
        // TOOD: maybe I should really create a Transforms class...
        let center = shape.object_to_world.col(3).truncate();
        let radius = sphere.radius;
        let area = Self::area_calc(radius);

        Self {
            center,
            radius,
            area,
            bh_index: 0,
        }
    }

    pub fn new_mock(origin: Vec3, radius: f32) -> Self {
        Sphere {
            center: origin,
            radius,
            area: Self::area_calc(radius),
            bh_index: 0,
        }
    }

    pub fn hit(&self, ray: &Ray) -> Option<ShapeHitInfo> {
        let oo = ray.orig - self.center;
        // PBRT always uses f64 for precision here
        let a = ray.dir.length_squared() as f64;
        // b = 2h -> quadratic formula can be simplified
        let half_b = ray.dir.dot(oo) as f64;
        let c = (oo.length_squared() - sqr(self.radius)) as f64;

        let discriminant = sqr(half_b) - a * c;
        let t = if discriminant < 0. {
            return None;
        } else {
            let t0 = (-half_b + discriminant.sqrt()) / a;
            let t1 = (-half_b - discriminant.sqrt()) / a;
            t0.min(t1)
        };

        let pos = ray.orig + ray.dir * t as f32;
        let normal = (pos - self.center).normalize();
        // TODO: sphere UVs

        Some(ShapeHitInfo::new(pos, normal, t as f32, None))
    }

    pub fn sample_point(&self, rng: &mut SmallRng) -> ShapeSample {
        let sample_dir = sampling::sample_uniform_sphere(rng);
        let pos = self.center + self.radius * sample_dir;
        ShapeSample::new(pos, sample_dir)
    }

    pub fn aabb(&self) -> AABB {
        let a = self.center - self.radius;
        let b = self.center + self.radius;
        AABB::new(a, b)
    }

    pub fn area(&self) -> f32 {
        self.area
    }

    fn area_calc(radius: f32) -> f32 {
        4. * PI * sqr(radius)
    }

    pub fn set_bh_node_index(&mut self, i: usize) {
        self.bh_index = i;
    }

    pub fn bh_node_index(&self) -> usize {
        self.bh_index
    }
}

#[cfg(test)]
mod test_super {
    use super::*;
    use glam::vec3;

    #[test]
    fn test_sphere_intersection() {
        let sphere = Sphere::new_mock(vec3(0., 0., 1.), 1.);

        let ray_hit = Ray::new(vec3(0., 0., -1.), vec3(0., 0., 1.));
        let hitinfo = sphere.hit(&ray_hit).unwrap();
        assert_eq!(hitinfo.t, 1.);

        let ray_nohit = Ray::new(vec3(0., 0., -1.), vec3(1., 0., 0.));
        let hitinfo = sphere.hit(&ray_nohit);
        assert!(hitinfo.is_none());
    }
}
