use std::f32::consts::PI;

use glam::{vec2, vec3, Vec2, Vec3};
use rand::{distributions::Uniform, prelude::Distribution, rngs::SmallRng};

use crate::{
    math::{self, sqr},
    vecmath::orient_dir,
};

/// Sampling: https://pbr-book.org/3ed-2018/Monte_Carlo_Integration/2D_Sampling_with_Multidimensional_Transformations#UniformlySamplingaHemisphere
pub fn sample_uniform_hemisphere(rng: &mut SmallRng) -> Vec3 {
    // Coordinate frame: https://pbr-book.org/3ed-2018/Reflection_Models/Specular_Reflection_and_Transmission
    let dist = Uniform::from(0f32..1f32);
    let u = dist.sample(rng);
    let v = dist.sample(rng);

    let z = u;
    let r = f32::sqrt(0f32.max(1. - z * z));
    let phi = 2. * PI * v;
    Vec3::new(r * f32::cos(phi), r * f32::sin(phi), z).normalize()
}

pub fn sample_uniform_sphere(rng: &mut SmallRng) -> Vec3 {
    let dist = Uniform::from(0f32..1f32);
    let u = dist.sample(rng);
    let v = dist.sample(rng);

    let z = 1. - 2. * u;
    let r = f32::sqrt(0f32.max(1. - sqr(z)));
    let phi = 2. * PI * v;
    Vec3::new(r * phi.cos(), r * phi.sin(), z).normalize()
}

pub fn sample_cosine_hemisphere(rng: &mut SmallRng) -> Vec3 {
    let dist = Uniform::from(0f32..1f32);
    let u = dist.sample(rng);
    let v = dist.sample(rng);

    let d = sample_uniform_disk_concentric(vec2(u, v));
    let z = math::safe_sqrt(1. - d.x * d.x - d.y * d.y);
    vec3(d.x, d.y, z)
}

fn sample_uniform_disk_concentric(u: Vec2) -> Vec2 {
    // Map _u_ to $[-1,1]^2$ and handle degeneracy at the origin
    let u_offset = 2. * u - vec2(1., 1.);
    if u_offset.x == 0. && u_offset.y == 0. {
        return vec2(0., 0.);
    }

    // Apply concentric mapping to point
    let theta: f32;
    let r: f32;
    if u_offset.x.abs() > u_offset.y.abs() {
        r = u_offset.x;
        theta = (PI / 4.) * (u_offset.y / u_offset.x);
    } else {
        r = u_offset.y;
        theta = (PI / 2.) - (PI / 4.) * (u_offset.x / u_offset.y);
    }
    r * vec2(theta.cos(), theta.sin())
}

/// Taken from "Real Shading in Unreal Engine 4".
pub fn sample_trowbridge_reitz(rng: &mut SmallRng, normal: Vec3, roughness: f32) -> Vec3 {
    let a = roughness;

    let dist = Uniform::from(0f32..1f32);
    let u = dist.sample(rng);
    let v = dist.sample(rng);
    let xi = vec2(u, v);

    let phi = 2.0 * PI * xi.x;
    let cos_theta = math::safe_sqrt((1. - xi.y) / (1. + (a * a - 1.) * xi.y));
    let sin_theta = math::safe_sqrt(1. - cos_theta * cos_theta);

    // from spherical coordinates to cartesian coordinates - halfway vector
    let halfway = vec3(phi.cos() * sin_theta, phi.sin() * sin_theta, cos_theta).normalize();
    // from tangent-space H vector to world-space sample vector
    orient_dir(halfway, normal)
}

/// Samples the CMF, return an index into the CMF slice.
/// Expects a normalized CMF.
pub fn sample_discrete_cmf(cmf: &[f32], rng: &mut SmallRng) -> usize {
    let dist = Uniform::from(0f32..1f32);
    let u = dist.sample(rng);

    cmf.into_iter().position(|cum_prob| u < *cum_prob).unwrap()
}

/// Taken from PBRT - UniformSampleTriangle.
/// Return barycentric coordinates that can be used to sample any triangle.
pub fn sample_uniform_triangle(rng: &mut SmallRng) -> [f32; 3] {
    let dist = Uniform::from(0f32..1f32);
    let u = dist.sample(rng);
    let v = dist.sample(rng);

    let sqrt_u = u.sqrt();

    let b0 = 1. - sqrt_u;
    let b1 = v * sqrt_u;
    let b2 = 1. - b0 - b1;

    debug_assert_eq!(b0 + b1 + b2, 1.);

    [b0, b1, b2]
}
