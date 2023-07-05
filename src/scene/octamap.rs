use std::path::Path;

use eyre::Result;
use glam::{vec2, vec3, Vec2, Vec3};

use crate::{
    color::color_space::ColorSpace,
    math::{safe_sqrt, sqr},
};

/// Octahedral map texture
pub struct OctaMap {
    width: usize,
    height: usize,
    pixels: Vec<Vec3>,
    color_space: ColorSpace,
}

impl OctaMap {
    pub fn load(path: &Path) -> Result<Self> {
        let image = exr::prelude::read_first_rgba_layer_from_file(
            path,
            |resolution, _| {
                let size = resolution.width() * resolution.height();
                // TODO: should check if sides are euqal ?

                Self {
                    width: resolution.width(),
                    height: resolution.height(),
                    pixels: vec![Vec3::ZERO; size],
                    color_space: ColorSpace::Srgb,
                }
            },
            |pixels, position, (r, g, b, _): (f32, f32, f32, f32)| {
                pixels.set(position.x(), position.y(), vec3(r, g, b));
            },
        )?;

        if let Some(_chromaticities) = image.attributes.chromaticities {
            todo!("Deal with EXR images that aren't sRGB");
        }

        Ok(image.layer_data.channel_data.pixels)
    }

    fn set(&mut self, x: usize, y: usize, val: Vec3) {
        self.pixels[y * self.width + x] = val;
    }

    fn get(&self, x: usize, y: usize) -> Vec3 {
        let y = self.height - 1 - y;
        self.pixels[y * self.width + x]
    }

    pub fn sample(&self, dir: Vec3) -> Vec3 {
        let [x, y] = self.sphere_to_square(dir).to_array();

        let x = (x * ((self.width - 1) as f32)) as usize;
        let y = (y * ((self.height - 1) as f32)) as usize;

        self.get(x, y)
    }

    /// Code taken from PBRTv4.
    /// Via source code from Clarberg: Fast Equal-Area Mapping of the (Hemi)Sphere using SIMD.
    pub fn sphere_to_square(&self, dir: Vec3) -> Vec2 {
        // Change coordinates from world-space to paper-space
        let dir = vec3(dir.x, dir.z, dir.y);
        debug_assert!(dir.is_normalized());
        debug_assert!(sqr(dir.length()) > 0.999 && sqr(dir.length()) < 1.001);
        let [x, y, z] = dir.abs().to_array();

        // Compute the radius r
        let r = safe_sqrt(1. - z);
        // Compute the argument to atan (detect a=0 to avoid div-by-zero)
        let a = x.max(y);
        let mut b = x.min(y);
        b = if a == 0. { 0. } else { b / a };

        // Polynomial approximation of atan(x)*2/pi, x=b
        // Coefficients for 6th degree minimax approximation of atan(x)*2/pi,
        // x=[0,1].
        const T1: f32 = 0.406758566246788489601959989e-5;
        const T2: f32 = 0.636226545274016134946890922156;
        const T3: f32 = 0.61572017898280213493197203466e-2;
        const T4: f32 = -0.247333733281268944196501420480;
        const T5: f32 = 0.881770664775316294736387951347e-1;
        const T6: f32 = 0.419038818029165735901852432784e-1;
        const T7: f32 = -0.251390972343483509333252996350e-1;

        let mut phi = T6 + T7 * b;
        phi = T5 + phi * b;
        phi = T4 + phi * b;
        phi = T3 + phi * b;
        phi = T2 + phi * b;
        phi = T1 + phi * b;

        // Extend phi if the input is in the range 45-90 degrees (u<v)
        if x < y {
            phi = 1. - phi;
        }

        // Find (u,v) based on (r,phi)
        let mut v = phi * r;
        let mut u = r - v;

        if dir.z < 0. {
            // southern hemisphere -> mirror u,v
            let tmp = u;
            u = v;
            v = tmp;

            u = 1. - u;
            v = 1. - v;
        }

        // Move (u,v) to the correct quadrant based on the signs of (x,y)
        u = f32::copysign(u, dir.x);
        v = f32::copysign(v, dir.y);

        // Transform (u,v) from [-1,1] to [0,1]
        vec2(0.5 * (u + 1.), 0.5 * (v + 1.))
    }

    pub fn color_space(&self) -> &ColorSpace {
        &self.color_space
    }
}

#[cfg(test)]
mod test_super {
    use crate::vecmath::{spherical_to_cartesian, vec3_cmp_assert};

    use super::*;

    #[test]
    fn test_sphere_to_square() {
        let octamap = OctaMap::load(&Path::new("resources/test/equalareatest.exr")).unwrap();

        const DARKER_BLUE: Vec3 = vec3(0.01227, 0.462086, 0.665376);
        const LIGHT_BLUE: Vec3 = vec3(0.309455, 0.603845, 0.846861);
        const GREEN: Vec3 = vec3(0.010317, 0.708381, 0.057810);
        const YELLOW: Vec3 = vec3(0.708380, 0.693867, 0.038214);
        const WHITE: Vec3 = vec3(1., 1., 1.);
        const RED: Vec3 = vec3(0.603840, 0.048164, 0.034344);
        const BROWN: Vec3 = vec3(0.376268, 0.149956, 0.010333);
        const MAGENTA: Vec3 = vec3(0.571130, 0.023152, 0.603818);

        // Upper hemisphere
        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                45f32.to_radians(),
                45f32.to_radians(),
            )),
            BROWN,
        );

        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                45f32.to_radians(),
                135f32.to_radians(),
            )),
            WHITE,
        );

        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                45f32.to_radians(),
                225f32.to_radians(),
            )),
            YELLOW,
        );

        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                45f32.to_radians(),
                315f32.to_radians(),
            )),
            RED,
        );

        // Lower hemisphere
        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                135f32.to_radians(),
                45f32.to_radians(),
            )),
            MAGENTA,
        );

        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                135f32.to_radians(),
                135f32.to_radians(),
            )),
            LIGHT_BLUE,
        );

        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                135f32.to_radians(),
                225f32.to_radians(),
            )),
            GREEN,
        );

        vec3_cmp_assert(
            octamap.sample(spherical_to_cartesian(
                135f32.to_radians(),
                315f32.to_radians(),
            )),
            DARKER_BLUE,
        );
    }
}
