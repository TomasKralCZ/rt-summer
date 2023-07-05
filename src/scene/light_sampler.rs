use rand::rngs::SmallRng;

use crate::{sampling::sample_discrete_cmf, util::TaggedPtr};

use super::{primitive::Primitive, Light, LightSample};

pub struct LightSampler {
    total_area: f32,
    lights_cmf: Vec<f32>,
    lights_pmf: Vec<f32>,
}

impl LightSampler {
    pub fn new(primitives: &[TaggedPtr<Primitive>], lights: &[Light]) -> Self {
        let total_area = lights.iter().map(|l| primitives[l.primitive].area()).sum();

        let primitive_area_ratios: Vec<f32> = lights
            .iter()
            .map(|l| primitives[l.primitive].area() / total_area)
            .collect();

        debug_assert_eq!(primitive_area_ratios.iter().sum::<f32>(), 1.);

        let mut primitives_cmf = primitive_area_ratios.clone();

        // Calculate the CMF
        let mut sum = 0f32;
        for p in &mut primitives_cmf {
            let sum_before = sum;
            sum += *p;
            *p = *p + sum_before;
        }

        debug_assert_eq!(primitives_cmf.last(), Some(&1.));

        Self {
            total_area,
            lights_cmf: primitives_cmf,
            lights_pmf: primitive_area_ratios,
        }
    }

    pub fn sample<'s>(
        &'s self,
        primitives: &[TaggedPtr<Primitive>],
        lights: &'s [Light],
        rng: &mut SmallRng,
    ) -> Option<LightSample> {
        if self.lights_cmf.len() > 0 {
            let sampled_light = sample_discrete_cmf(&self.lights_cmf, rng);
            let pmf = self.lights_pmf[sampled_light];
            let light = &lights[sampled_light];

            let primitive = &primitives[light.primitive];

            Some(LightSample::new(
                primitive.sample_point(rng),
                &light.emission,
                primitive.area(),
                pmf,
            ))
        } else {
            None
        }
    }
}
