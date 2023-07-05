use std::f32::consts::PI;

use glam::Vec3;
use rand::rngs::SmallRng;

use crate::{
    color::spectrum::{SampledWavelengths, SpectralQuantity},
    integrator::shading_geometry::ShadingGeometry,
    pbrt_loader::scene_description::{ConductorMaterial, Material},
    sampling, vecmath,
};

pub struct Bxdf<'m> {
    mat: &'m Material,
    rng: &'m mut SmallRng,
}

impl<'m> Bxdf<'m> {
    pub fn new(mat: &'m Material, rng: &'m mut SmallRng) -> Self {
        Self { mat, rng }
    }

    pub fn sample(&mut self, normal: Vec3, view_dir: Vec3) -> Vec3 {
        match self.mat {
            Material::Diffuse(_) => {
                let sample_dir = sampling::sample_cosine_hemisphere(self.rng);
                vecmath::orient_dir(sample_dir, normal)
            }
            Material::Conductor(material) => {
                // TODO: better sampling algorithm
                let halfway = sampling::sample_trowbridge_reitz(
                    self.rng,
                    normal,
                    material.roughness.vroughness,
                );
                (2. * view_dir.dot(halfway) * halfway - view_dir).normalize()
            }
        }
    }

    pub fn pdf(&mut self, sgeom: &ShadingGeometry) -> f32 {
        let pdf = match self.mat {
            Material::Diffuse(_) => sgeom.cos_theta / PI,
            Material::Conductor(material) => {
                let d = distribution_trowbridge_reitz(sgeom.noh, material.roughness.vroughness);
                let mut res = d * sgeom.noh / (4. * sgeom.hov);
                if res <= 0. {
                    res = -res;
                }

                res
            }
        };

        debug_assert!(pdf > 0.);
        pdf
    }

    pub fn eval(
        &mut self,
        sgeom: &ShadingGeometry,
        sampled_lambdas: &SampledWavelengths,
    ) -> SpectralQuantity {
        let brdf = match self.mat {
            Material::Diffuse(diffuse_mat) => {
                let brdf = sampled_lambdas
                    .lambdas
                    .map(|lambda| diffuse_mat.reflectance.eval_single(lambda) / PI);
                SpectralQuantity::new(brdf)
            }
            Material::Conductor(conductor_mat) => {
                let brdf = sampled_lambdas
                    .lambdas
                    .map(|lambda| eval_conductor_brdf(lambda, conductor_mat, sgeom));
                SpectralQuantity::new(brdf)
            }
        };

        debug_assert!(brdf.vals.iter().all(|brdf| *brdf >= 0.));
        brdf
    }
}

fn distribution_trowbridge_reitz(noh: f32, roughness: f32) -> f32 {
    let asq = roughness * roughness;
    let denom = (noh * noh) * (asq - 1.) + 1.;

    asq / (PI * denom * denom)
}

fn fresnel_schlick(f0: Vec3, voh: f32) -> Vec3 {
    f0 + (1. - f0) * f32::powi(f32::clamp(1. - voh, 0.0, 1.0), 5)
}

fn visibility_smith_height_correlated_ggx(nov: f32, nol: f32, roughness: f32) -> f32 {
    let asq = roughness * roughness;
    let nov_sq = nov * nov;
    let nol_sq = nol * nol;

    let denoml = nol * f32::sqrt(asq + nov_sq * (1. - asq));
    let denomv = nov * f32::sqrt(asq + nol_sq * (1. - asq));

    // Protect against division by zero
    0.5 / (denoml + denomv + 0.00001)
}

fn eval_conductor_brdf(lambda: f32, mat: &ConductorMaterial, sgeom: &ShadingGeometry) -> f32 {
    // TODO: support anisotropic version
    assert_eq!(mat.roughness.vroughness, mat.roughness.uroughness);

    let roughness = mat.roughness.uroughness;

    let visibility = visibility_smith_height_correlated_ggx(sgeom.nov, sgeom.cos_theta, roughness);
    let dist = distribution_trowbridge_reitz(sgeom.noh, roughness);
    //let fresnel = fresnel_schlick(f0, sgeom.hov);
    let fresnel = 1.;

    visibility * dist * fresnel
}
