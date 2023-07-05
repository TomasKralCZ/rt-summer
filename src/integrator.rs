use eyre::{eyre, Result};
use glam::Vec3;
use rand::{distributions::Uniform, prelude::Distribution, rngs::SmallRng};
use rgb2spec::RGB2Spec;

use crate::{
    bxdf::Bxdf,
    color::spectrum::{rgb_spectrum::RGBTOSPEC, SampledWavelengths, SpectralQuantity},
    geometry::Ray,
    math::sqr,
    scene::{HitInfo, Scene},
};

pub mod shading_geometry;

use shading_geometry::ShadingGeometry;

pub enum Integrator {
    RandomWalk(RandomWalkIntegrator),
    SimplePath(SimplePathIntegrator),
}

impl Integrator {
    pub fn new(kind: &str) -> Result<Self> {
        Ok(match kind {
            "random-walk" => Self::RandomWalk(RandomWalkIntegrator),
            "simple-path" => Self::SimplePath(SimplePathIntegrator),
            _ => return Err(eyre!("Unknown integrator kind: '{}'", kind)),
        })
    }

    pub fn ray_l(
        &self,
        ray: &Ray,
        sampled_lambdas: &mut SampledWavelengths,
        scene: &Scene,
        rng: &mut SmallRng,
    ) -> SpectralQuantity {
        let rgbtospec = RGBTOSPEC.get().unwrap();

        match self {
            Integrator::RandomWalk(_) => RandomWalkIntegrator::ray_l(
                ray,
                sampled_lambdas,
                scene,
                rng,
                rgbtospec,
                0,
                SpectralQuantity::ONE,
            ),
            Integrator::SimplePath(_) => SimplePathIntegrator::ray_l_iter(
                ray.clone(),
                sampled_lambdas,
                scene,
                rng,
                rgbtospec,
            ),
        }
    }
}

pub struct RandomWalkIntegrator;

impl RandomWalkIntegrator {
    fn ray_l(
        hit_ray: &Ray,
        sampled_lambdas: &mut SampledWavelengths,
        scene: &Scene,
        rng: &mut SmallRng,
        rgbtospec: &RGB2Spec,
        mut depth: u32,
        mut throughput: SpectralQuantity,
    ) -> SpectralQuantity {
        depth += 1;

        if let Some(mut hitinfo) = scene.trace_ray(hit_ray) {
            let mut emission = hitinfo
                .light
                .map(|light_id| scene.lights[light_id].emission.eval(sampled_lambdas))
                .unwrap_or(SpectralQuantity::ZERO);

            hitinfo.normal = hitinfo.normal.normalize();
            if -hit_ray.dir.dot(hitinfo.normal) < 0. {
                emission = SpectralQuantity::ZERO;
                hitinfo.normal = -hitinfo.normal;
            }

            let mut bxdf = Bxdf::new(&hitinfo.material, rng);
            let sample_dir = bxdf.sample(hitinfo.normal, -hit_ray.dir);
            let next_ray = spawn_ray(&hitinfo, sample_dir);
            let sgeom = ShadingGeometry::new(&hitinfo.normal, &sample_dir, &hit_ray.dir);

            let pdf = bxdf.pdf(&sgeom);
            let bxdf_eval = bxdf.eval(&sgeom, sampled_lambdas);

            throughput *= bxdf_eval * sgeom.cos_theta * (1. / pdf);

            let roulette_compensation =
                if let Some(compensation) = russian_roulette(depth, rng, &throughput) {
                    compensation
                } else {
                    return emission;
                };

            throughput *= 1. / roulette_compensation;

            let li = Self::ray_l(
                &next_ray,
                sampled_lambdas,
                scene,
                rng,
                rgbtospec,
                depth,
                throughput,
            );
            let estimate_brdf_sample =
                li * bxdf_eval * sgeom.cos_theta * (1. / roulette_compensation);

            return emission + estimate_brdf_sample * (1. / pdf);
        } else {
            ray_nohit(hit_ray, scene, rgbtospec, &sampled_lambdas)
        }
    }
}

pub struct SimplePathIntegrator;

impl SimplePathIntegrator {
    fn ray_l_iter(
        hit_ray: Ray,
        sampled_lambdas: &mut SampledWavelengths,
        scene: &Scene,
        rng: &mut SmallRng,
        rgbtospec: &RGB2Spec,
    ) -> SpectralQuantity {
        let mut depth = 0;
        let mut throughput = SpectralQuantity::ONE;
        let mut radiance = SpectralQuantity::ZERO;
        let mut last_pdf_bxdf = 1f32;
        let mut ray = hit_ray;
        let mut last_pos = Vec3::ZERO;

        loop {
            let hit = scene.trace_ray(&ray);
            if hit.is_none() {
                let li = ray_nohit(&ray, scene, rgbtospec, sampled_lambdas);
                radiance += throughput * li;
                break;
            }

            let mut hitinfo = hit.unwrap();

            hitinfo.normal = hitinfo.normal.normalize();
            let backside = -ray.dir.dot(hitinfo.normal) < 0.;
            if backside {
                hitinfo.normal = -hitinfo.normal;
            }

            if let Some(light) = hitinfo.light {
                let light = &scene.lights[light];
                let emission = if backside {
                    SpectralQuantity::ZERO
                } else {
                    light.emission.eval(sampled_lambdas)
                };

                if depth == 0 {
                    radiance += throughput * emission;
                } else {
                    let p_to_l_norm = (hitinfo.pos - last_pos).normalize();
                    let p_to_l_mag_sq = (hitinfo.pos - last_pos).length_squared();
                    let cos_light = hitinfo.normal.dot(-p_to_l_norm);

                    let pdf_light = p_to_l_mag_sq
                        / (scene.light_area(&light) * cos_light * scene.lights.len() as f32);
                    let bxdf_weight = Self::mis_power_heuristic(last_pdf_bxdf, pdf_light);

                    radiance += throughput * bxdf_weight * emission;
                }
            }

            let mut bxdf = Bxdf::new(&hitinfo.material, rng);
            let sample_dir = bxdf.sample(hitinfo.normal, -ray.dir);
            let bxdf_ray = spawn_ray(&hitinfo, sample_dir);
            let sgeom_bxdf = ShadingGeometry::new(&hitinfo.normal, &sample_dir, &ray.dir);

            let pdf_bxdf = bxdf.pdf(&sgeom_bxdf);
            let bxdf_eval = bxdf.eval(&sgeom_bxdf, sampled_lambdas);

            if let Some(light_s) = scene.sample_light(rng) {
                let light_pos = light_s.shape_sample.pos;
                let p_to_l_norm = (light_pos - hitinfo.pos).normalize();
                let p_to_l_mag_sq = (light_pos - hitinfo.pos).length_squared();

                let cos_light = light_s.shape_sample.normal.dot(-p_to_l_norm);
                let sgeom_light = ShadingGeometry::new(&hitinfo.normal, &p_to_l_norm, &ray.dir);

                if sgeom_light.cos_theta > 0. && cos_light > 0. {
                    let visibility = scene.is_unoccluded(bxdf_ray.orig, light_pos);

                    if visibility {
                        let pdf_light = light_s.pmf * p_to_l_mag_sq / (light_s.area * cos_light);
                        let mut bxdf = Bxdf::new(&hitinfo.material, rng);
                        let bxdf_light_eval = bxdf.eval(&sgeom_light, sampled_lambdas);

                        let weight_light =
                            Self::mis_power_heuristic(pdf_light, bxdf.pdf(&sgeom_light));
                        let light_emission = light_s.emission.eval(sampled_lambdas);

                        radiance += bxdf_light_eval
                            * light_emission
                            * weight_light
                            * throughput
                            * sgeom_light.cos_theta
                            * (1. / pdf_light);
                    }
                }
            }

            match russian_roulette(depth, rng, &throughput) {
                Some(compensation) => throughput *= 1. / compensation,
                None => break,
            };

            depth += 1;
            throughput *= bxdf_eval * sgeom_bxdf.cos_theta * (1. / pdf_bxdf);
            last_pdf_bxdf = pdf_bxdf;
            ray = bxdf_ray;
            last_pos = hitinfo.pos;
        }

        radiance
    }

    /// Adapted from PBRT. Specific case where 1 sample is taken from each distribution.
    fn mis_power_heuristic(fpdf: f32, gpdf: f32) -> f32 {
        sqr(fpdf) / (sqr(fpdf) + sqr(gpdf))
    }

    fn mis_balance_heuristic(fpdf: f32, gpdf: f32) -> f32 {
        fpdf / (fpdf + gpdf)
    }
}

fn spawn_ray(hitinfo: &HitInfo, dir: Vec3) -> Ray {
    // TODO: more robust floating-point error handling when spawning rays
    let ray_orig = hitinfo.pos + 0.001 * hitinfo.normal;
    let ray_orig = hitinfo.pos + 0.008 * hitinfo.normal;
    Ray::new(ray_orig, dir)
}

/// Randomly selects if a ray should be terminated based on its throughput.
/// Roulette is only applied after the first 3 bounces.
/// If ray shoould NOT be terminated, the roulette compensation is returned.
fn russian_roulette(depth: u32, rng: &mut SmallRng, throughput: &SpectralQuantity) -> Option<f32> {
    if depth > 3 {
        let dist = Uniform::from(0f32..1f32);
        let u = dist.sample(rng);
        let survival_prob = 1. - throughput.max_value().max(0.05);

        if u < survival_prob {
            None
        } else {
            let roulette_compensation = 1. - survival_prob;
            Some(roulette_compensation)
        }
    } else {
        Some(1.)
    }
}

fn ray_nohit(
    ray: &Ray,
    scene: &Scene,
    rgbtospec: &RGB2Spec,
    lambdas: &SampledWavelengths,
) -> SpectralQuantity {
    if let Some(infinite_light) = &scene.infinite_light {
        let rgbspectrum = infinite_light.sample(ray.dir, rgbtospec);
        rgbspectrum.eval(lambdas)
    } else {
        SpectralQuantity::ZERO
    }
}
