use std::path::PathBuf;

use glam::{Mat4, Vec2, Vec3};
use rgb2spec::RGB2Spec;

use crate::color::{
    color_space::ColorSpace,
    spectrum::rgb_spectrum::{RgbSpectrum, RgbSpectrumKind},
};

// TODO: support loading general spectra, not just RGBSpectrum...

#[derive(Debug)]
pub struct SceneDescription {
    pub options: ScreenWideOptions,
    pub shapes: Vec<ShapeWithParams>,
    pub infinite_light: Option<InfiniteLightSource>,
}

#[derive(Debug, Default)]
pub struct ScreenWideOptions {
    pub general_options: RenderingOptions,
    pub camera: Camera,
    pub sampler: Sampler,
    pub film: Film,
    pub filter: PixelFilter,
}

#[derive(Debug)]
pub struct Camera {
    pub typ: CameraTyp,
    pub fov: f32,
    pub camera_from_world_transform: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            typ: CameraTyp::Perspective,
            fov: 90.,
            camera_from_world_transform: Mat4::ZERO,
        }
    }
}

#[derive(Debug)]
pub enum CameraTyp {
    Orthographic,
    Perspective,
    Realistic,
    Spherical,
}

#[derive(Debug)]
pub struct Sampler {
    pub typ: SamplerTyp,
    pub pixel_samples: i32,
}

impl Default for Sampler {
    fn default() -> Self {
        Self {
            typ: SamplerTyp::ZSobol,
            pixel_samples: 16,
        }
    }
}

#[derive(Debug)]
pub enum SamplerTyp {
    Halton,
    ZSobol,
}

#[derive(Debug)]
pub struct Film {
    pub typ: FilmType,
    pub xresolution: i32,
    pub yresolution: i32,
    pub filename: String,
}

impl Default for Film {
    fn default() -> Self {
        Self {
            typ: FilmType::Rgb,
            xresolution: 1280,
            yresolution: 720,
            filename: String::from("pbrt.exr"),
        }
    }
}

#[derive(Debug)]
pub enum FilmType {
    Rgb,
    GBuffer,
    Spetral,
}

#[derive(Debug)]
pub struct PixelFilter {
    typ: PixelFilterType,
    radius: f32,
}

impl Default for PixelFilter {
    fn default() -> Self {
        Self {
            typ: PixelFilterType::Gaussian,
            radius: 1.5,
        }
    }
}

#[derive(Debug)]
enum PixelFilterType {
    Box,
    Gaussian,
    Mitchell,
    Sin,
    Triangle,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct RenderingOptions {
    disablepixeljitter: bool,
    disabletexturefiltering: bool,
    disablewavelengthjitter: bool,
    displacementedgescale: f32,
    msereferenceimage: String,
    msereferenceout: String,
    rendercoordsys: String,
    seed: i32,
    forcediffuse: bool,
    pixelstats: bool,
    wavefront: bool,
}

impl Default for RenderingOptions {
    fn default() -> Self {
        Self {
            disablepixeljitter: false,
            disabletexturefiltering: false,
            disablewavelengthjitter: false,
            displacementedgescale: 1.,
            msereferenceimage: String::new(),
            msereferenceout: String::new(),
            rendercoordsys: "cameraworld".to_string(),
            seed: 0,
            forcediffuse: false,
            pixelstats: false,
            wavefront: false,
        }
    }
}

#[derive(Debug)]
pub struct ShapeWithParams {
    pub shape: Shape,
    pub material: Material,
    pub area_light: Option<AreaLightSource>,
    pub object_to_world: Mat4,
    pub reverse_normals: bool,
}

impl ShapeWithParams {
    pub fn new(
        shape: Shape,
        material: Material,
        area_light: Option<AreaLightSource>,
        object_to_world: Mat4,
        reverse_normals: bool,
    ) -> Self {
        Self {
            shape,
            material,
            area_light,
            object_to_world,
            reverse_normals,
        }
    }
}

#[derive(Debug)]
pub enum Shape {
    TriMesh(TriMesh),
    Sphere(Sphere),
}

#[derive(Debug, Clone)]
pub struct TriMesh {
    pub indices: Vec<i32>,
    pub pos: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
    pub tangents: Option<Vec<Vec3>>,
    pub uvs: Option<Vec<Vec2>>,
}

impl TriMesh {
    pub fn new(
        indices: Vec<i32>,
        pos: Vec<Vec3>,
        normals: Option<Vec<Vec3>>,
        tangents: Option<Vec<Vec3>>,
        uvs: Option<Vec<Vec2>>,
    ) -> Self {
        Self {
            indices,
            pos,
            normals,
            tangents,
            uvs,
        }
    }
}

#[derive(Debug)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

#[derive(Debug, Clone)]
pub struct AreaLightSource {
    /// Spectral distribution of the light's emitted radiance.
    pub radiance: RgbSpectrum,
}

impl AreaLightSource {
    pub fn new(radiance: RgbSpectrum) -> Self {
        Self { radiance }
    }

    pub fn new_default(rgbtospec: &RGB2Spec, color_space: ColorSpace) -> Self {
        Self {
            radiance: RgbSpectrum::new(
                rgbtospec,
                Vec3::ONE,
                RgbSpectrumKind::new_illuminant(color_space),
            ),
        }
    }
}

#[derive(Debug)]
pub enum LightSource {
    Infinite(InfiniteLightSource),
}

#[derive(Debug)]
pub struct InfiniteLightSource {
    pub scale: f32,
    pub filepath: PathBuf,
}

impl InfiniteLightSource {
    pub fn new(scale: f32, filepath: PathBuf) -> Self {
        Self { scale, filepath }
    }
}

#[derive(Debug, Clone)]
pub enum Material {
    Diffuse(DiffuseMaterial),
    Conductor(ConductorMaterial),
}

impl Material {
    pub fn new_default(rgbtospec: &RGB2Spec) -> Self {
        Self::Diffuse(DiffuseMaterial::new(rgbtospec, Vec3::splat(0.5)))
    }

    pub fn new_empty() -> Self {
        Self::Diffuse(DiffuseMaterial {
            reflectance: RgbSpectrum::new_empty(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DiffuseMaterial {
    pub reflectance: RgbSpectrum,
}

impl DiffuseMaterial {
    pub fn new(rgbtospec: &RGB2Spec, reflectance: Vec3) -> Self {
        Self {
            reflectance: RgbSpectrum::new(rgbtospec, reflectance, RgbSpectrumKind::Reflectance),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConductorMaterial {
    pub ior: RgbSpectrum,
    pub absorbtion_k: RgbSpectrum,
    pub roughness: MaterialRoughness,
}

impl ConductorMaterial {
    pub fn new(
        rgbtospec: &RGB2Spec,
        ior: Vec3,
        absorbtion_k: Vec3,
        roughness: MaterialRoughness,
    ) -> Self {
        Self {
            ior: RgbSpectrum::new(rgbtospec, ior, RgbSpectrumKind::Unbounded),
            absorbtion_k: RgbSpectrum::new(rgbtospec, absorbtion_k, RgbSpectrumKind::Unbounded),
            roughness,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MaterialRoughness {
    pub vroughness: f32,
    pub uroughness: f32,
}

impl MaterialRoughness {
    pub fn new(vroughness: f32, uroughness: f32) -> Self {
        Self {
            vroughness,
            uroughness,
        }
    }
}
