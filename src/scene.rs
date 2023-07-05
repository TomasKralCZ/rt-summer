use std::sync::Arc;

use eyre::Result;
use glam::{Vec2, Vec3};
use rand::rngs::SmallRng;
use rgb2spec::RGB2Spec;

use crate::{
    bvh::Bvh,
    color::spectrum::rgb_spectrum::{RgbSpectrum, RgbSpectrumKind},
    geometry::{
        sphere::Sphere,
        trianglemesh::{Triangle, TriangleMesh},
        Ray, Shape, ShapeHitInfo,
    },
    pbrt_loader::scene_description::{self, InfiniteLightSource, Material, SceneDescription},
    scene::primitive::{
        LightPrimitive, MeshTriangleLightPrimitive, MeshTrianglePrimitive, SimplePrimtive,
    },
    util::TaggedPtr,
};

use self::{light_sampler::LightSampler, octamap::OctaMap, primitive::Primitive};

mod light_sampler;
mod octamap;
pub mod primitive;

pub type LightId = usize;
pub type PrimitiveId = usize;

type SceneAlloc = std::alloc::Global;
const SCENE_ALLOC: std::alloc::Global = std::alloc::Global;

pub struct Scene {
    pub infinite_light: Option<InfiniteLight>,
    pub lights: Vec<Light, SceneAlloc>,
    /// TODO: custom allocator for Arc https://github.com/rust-lang/rust/pull/89132
    triangle_meshes: Vec<Arc<TriangleMesh>, SceneAlloc>,
    primitives: Vec<TaggedPtr<Primitive>, SceneAlloc>,
    bvh: Bvh,
    light_sampler: LightSampler,
}

impl Scene {
    pub fn init(scene_desc: SceneDescription) -> Result<Self> {
        let mut lights = Vec::new_in(SCENE_ALLOC);
        let mut triangle_meshes = Vec::new_in(SCENE_ALLOC);
        let mut primitives = Vec::new_in(SCENE_ALLOC);

        // TODO: calculate primitives len up front
        // TODO: benchmark creating the BVH
        // let triangle_count: usize = triangle_meshes.iter().map(|tm| tm.triangle_count()).sum();

        for shape_with_params in scene_desc.shapes {
            match shape_with_params.shape {
                scene_description::Shape::TriMesh(mesh) => {
                    let trimesh = Arc::new(TriangleMesh::new(
                        mesh,
                        Arc::new(shape_with_params.material),
                        shape_with_params.reverse_normals,
                    ));

                    for triangle_id in 0..trimesh.triangle_count() {
                        let triangle = Triangle::new(Arc::clone(&trimesh), triangle_id as u64);

                        let primitive = if let Some(light) = &shape_with_params.area_light {
                            let l = Light::new(primitives.len(), light.radiance.clone());
                            let light_id = lights.len();
                            lights.push(l);

                            Primitive::MeshTriangleLight(Box::new(MeshTriangleLightPrimitive::new(
                                triangle, light_id,
                            )))
                        } else {
                            Primitive::MeshTriangle(Box::new(MeshTrianglePrimitive::new(triangle)))
                        };

                        primitives.push(TaggedPtr::new(primitive));
                    }

                    triangle_meshes.push(Arc::clone(&trimesh));
                }
                ref shape => {
                    let mut light_id = None;
                    if let Some(light) = &shape_with_params.area_light {
                        let l = Light::new(primitives.len(), light.radiance.clone());
                        light_id = Some(lights.len());
                        lights.push(l);
                    }

                    let shape = match shape {
                        scene_description::Shape::TriMesh(_) => unreachable!(),
                        scene_description::Shape::Sphere(ref sphere) => {
                            let sphere = Sphere::new(&shape_with_params, sphere);
                            TaggedPtr::new(Shape::Sphere(Box::new(sphere)))
                        }
                    };

                    let primitive = if let Some(light) = light_id {
                        Primitive::Light(Box::new(LightPrimitive::new(
                            shape,
                            Arc::new(shape_with_params.material),
                            light,
                        )))
                    } else {
                        Primitive::Simple(Box::new(SimplePrimtive::new(
                            shape,
                            Arc::new(shape_with_params.material),
                        )))
                    };

                    primitives.push(TaggedPtr::new(primitive));
                }
            }
        }

        let my_bvh = crate::bvh::Bvh::build(&mut primitives);

        // Fixup the light indices because building the BVH reorders primitives
        for (i, prim) in primitives.iter().enumerate() {
            prim.0.map_ref(|prim| match prim {
                Primitive::MeshTriangleLight(tri_light) => {
                    lights[tri_light.light()].primitive = i;
                }
                Primitive::Light(light_prim) => {
                    lights[light_prim.light()].primitive = i;
                }
                _ => (),
            });
        }

        let infinite_light = if let Some(ils) = scene_desc.infinite_light {
            Some(InfiniteLight::init(ils)?)
        } else {
            None
        };

        Ok(Self {
            infinite_light,
            triangle_meshes,
            light_sampler: LightSampler::new(&primitives, &lights),
            lights,
            primitives,
            bvh: my_bvh,
        })
    }

    pub fn trace_ray(&self, ray: &Ray) -> Option<HitInfo> {
        self.trace_ray_bounded(ray, f32::INFINITY)
    }

    pub fn trace_ray_bounded(&self, ray: &Ray, maxt: f32) -> Option<HitInfo> {
        self.bvh.intersect(ray, maxt, &self.primitives)
    }

    pub fn is_unoccluded(&self, start: Vec3, end: Vec3) -> bool {
        let dir = end - start;
        let ray = Ray::new(start, dir);

        match self.bvh.intersect(&ray, f32::INFINITY, &self.primitives) {
            Some(hit) => hit.t >= dir.length() - 0.01,
            None => true,
        }

        // FIXME: ray shortening seems to be off

        /* !self
        .bvh
        .intersect(&ray, dir.length(), &self.primitives)
        .is_some() */
    }

    pub fn sample_light(&self, rng: &mut SmallRng) -> Option<LightSample> {
        self.light_sampler
            .sample(&self.primitives, &self.lights, rng)
    }

    pub fn light_area(&self, light: &Light) -> f32 {
        self.primitives[light.primitive].area()
    }

    pub fn primitives(&self) -> &[TaggedPtr<Primitive>] {
        self.primitives.as_ref()
    }
}

#[derive(Debug)]
pub struct HitInfo {
    pub pos: Vec3,
    pub normal: Vec3,
    pub t: f32,
    pub uv: Option<Vec2>,
    pub light: Option<LightId>,
    pub material: Arc<Material>,
}

impl HitInfo {
    pub fn new(
        pos: Vec3,
        normal: Vec3,
        t: f32,
        uv: Option<Vec2>,
        light: Option<LightId>,
        material: Arc<Material>,
    ) -> Self {
        Self {
            pos,
            normal,
            t,
            uv,
            light,
            material,
        }
    }

    pub fn from_shape_hitinfo(
        shape_hitinfo: ShapeHitInfo,
        material: Arc<Material>,
        light: Option<LightId>,
    ) -> Self {
        Self {
            pos: shape_hitinfo.pos,
            normal: shape_hitinfo.normal,
            t: shape_hitinfo.t,
            uv: shape_hitinfo.uv,
            light,
            material,
        }
    }
}

pub struct ShapeSample {
    pub pos: Vec3,
    pub normal: Vec3,
}

impl ShapeSample {
    pub fn new(pos: Vec3, normal: Vec3) -> Self {
        Self { pos, normal }
    }
}

pub struct LightSample<'r> {
    pub shape_sample: ShapeSample,
    pub emission: &'r RgbSpectrum,
    pub area: f32,
    /// Probability of choosing this light
    pub pmf: f32,
}

impl<'r> LightSample<'r> {
    pub fn new(shape_sample: ShapeSample, emission: &'r RgbSpectrum, area: f32, pmf: f32) -> Self {
        Self {
            shape_sample,
            emission,
            area,
            pmf,
        }
    }
}

pub struct Light {
    /// Index to the objects Vec in scene
    pub primitive: PrimitiveId,
    pub emission: RgbSpectrum,
}

impl Light {
    pub fn new(obj: PrimitiveId, emission: RgbSpectrum) -> Self {
        Self {
            primitive: obj,
            emission,
        }
    }
}

pub struct InfiniteLight {
    iblmap: OctaMap,
    scale: f32,
}

impl InfiniteLight {
    pub fn init(ils: InfiniteLightSource) -> Result<Self> {
        Ok(Self {
            iblmap: OctaMap::load(&ils.filepath)?,
            scale: ils.scale,
        })
    }

    pub fn sample(&self, dir: Vec3, rgbtospec: &RGB2Spec) -> RgbSpectrum {
        let rgb = self.iblmap.sample(dir) * self.scale;
        let color_space = self.iblmap.color_space();
        let spectrum_kind = RgbSpectrumKind::new_illuminant(*color_space);
        RgbSpectrum::new(rgbtospec, rgb, spectrum_kind)
    }
}
