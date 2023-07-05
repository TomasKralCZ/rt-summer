use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eyre::{eyre, Result};
use glam::{Mat4, Vec2, Vec3};
use rgb2spec::RGB2Spec;
use smallvec::SmallVec;

use crate::{
    color::{
        color_space::ColorSpace,
        spectrum::rgb_spectrum::{self, RgbSpectrum, RgbSpectrumKind, RGBTOSPEC},
    },
    pbrt_loader::lexer::Lexeme,
    vecmath,
};

use self::{
    lexer::Lexer,
    params::{ListParam, ListParamValue, ParamList, SingleValueOrList, Value, ValueList, ValueVec},
    scene_description::{
        AreaLightSource, Camera, CameraTyp, ConductorMaterial, DiffuseMaterial, Film, FilmType,
        InfiniteLightSource, LightSource, Material, MaterialRoughness, SceneDescription,
        ScreenWideOptions, Shape, ShapeWithParams, Sphere, TriMesh,
    },
};

mod lexer;
mod params;
mod ply_mesh;
pub mod scene_description;

type Int = i32;

#[derive(Clone)]
struct GraphicsState<'t> {
    ctm: Mat4,
    reverse_orientation: bool,
    area_light_source: Option<AreaLightSource>,
    material: Option<&'t str>,
    color_space: ColorSpace,
}

impl<'t> Default for GraphicsState<'t> {
    fn default() -> Self {
        Self {
            ctm: Mat4::IDENTITY,
            reverse_orientation: false,
            area_light_source: None,
            material: None,
            color_space: ColorSpace::Srgb,
        }
    }
}

pub struct SceneLoader<'t, 'r> {
    lexer: Lexer<'t>,
    saved_gstates: Vec<GraphicsState<'t>>,
    gstate: GraphicsState<'t>,
    file_directory: PathBuf,
    materials: HashMap<&'t str, Material>,
    rgbtospec: &'r RGB2Spec,
}

impl<'t, 'r> SceneLoader<'t, 'r> {
    pub fn load_from_path<T: AsRef<Path>>(file: T) -> Result<SceneDescription>
    where
        PathBuf: From<T>,
    {
        rgb_spectrum::init_rgbtospec()?;

        let txt = std::fs::read_to_string(&file)?;
        if !txt.is_ascii() {
            return Err(eyre!("Input text contains non-ASCII characters"));
        }

        let mut file_path = PathBuf::from(file);
        file_path.pop();

        let rgbtospec = RGBTOSPEC.get().unwrap();

        let mut s = SceneLoader {
            lexer: Lexer::new(&txt),
            saved_gstates: Vec::new(),
            gstate: GraphicsState::default(),
            file_directory: file_path,
            materials: HashMap::new(),
            rgbtospec,
        };
        let scene = s.load()?;

        Ok(scene)
    }

    pub fn load(&mut self) -> Result<SceneDescription> {
        let options = self
            .parse_screen_wide_options()
            .inspect_err(|e| self.report_error(e))?;

        assert!(self.saved_gstates.is_empty());
        // TODO: anything else needs to be reset ?
        self.gstate.ctm = Mat4::IDENTITY;

        let (shapes, infinite_light) = self.parse_scene().inspect_err(|e| self.report_error(e))?;

        Ok(SceneDescription {
            options,
            shapes,
            infinite_light,
        })
    }

    fn report_error(&self, report: &eyre::Report) {
        eprintln!("Scene loading error: '{report}'");

        /* let start_i = self.i.saturating_sub(10);
        let end_i = usize::min(self.i + 10, self.txt.len());
        let context = &self.txt[start_i..end_i];
        let char_pointer = self.i - start_i + 1;

        println!("Context:\n'{context}'");
        let mut pointer = " ".repeat(char_pointer);
        pointer.push('^');
        println!("{pointer}"); */
    }

    //
    // Screen-wide options parsing
    //

    /// https://pbrt.org/fileformat-v4#scene-wide-rendering-options
    fn parse_screen_wide_options(&mut self) -> Result<ScreenWideOptions> {
        let mut screen_cam = None;
        let mut screen_film = None;

        loop {
            let dir = self.expect(Lexeme::Str(""))?.unwrap_str();
            match dir {
                // Options exclusive to pre-WorldBegin
                "Option" => todo!(),
                "Camera" => {
                    let cam = self.parse_camera()?;
                    if screen_cam.is_some() {
                        return Err(eyre!("Duplicate Camera definition"));
                    }
                    screen_cam = Some(cam);
                }
                "Sampler" => {
                    let _sampler = self.parse_sampler()?;
                    eprintln!("Sampler settings are ignored as of yet.");
                }
                "ColorSpace" => todo!(),
                "Film" => {
                    let film = self.parse_film()?;
                    if screen_film.is_some() {
                        return Err(eyre!("Duplicate Film definition"));
                    }
                    screen_film = Some(film);
                }
                "PixelFilter" => {
                    let _filter = self.parse_pixel_filter()?;
                    eprintln!("Filter settings are ignored as of yet.");
                }
                "Integrator" => {
                    self.parse_param_list()?;
                    eprintln!("Integrator setting is ignored");
                }
                "Accelerator" => todo!(),
                // WorldBegin
                "WorldBegin" => break,
                // Mediums
                "MakeNamedMedium" => todo!(),
                "MediumInterface" => todo!(),
                // Transformations
                "Transform" => self.parse_transform()?,
                "Scale" => self.parse_scale()?,
                "LookAt" => self.parse_look_at()?,
                option => return Err(eyre!("Unkown or unimplemented directive: '{}'", option)),
            }
        }

        let swo = ScreenWideOptions {
            camera: screen_cam.ok_or_else(|| eyre!("No Camera was provided"))?,
            film: screen_film.ok_or_else(|| eyre!("No Film was provided"))?,
            ..ScreenWideOptions::default()
        };

        Ok(swo)
    }

    fn parse_camera(&mut self) -> Result<Camera> {
        let mut cam = Camera {
            camera_from_world_transform: self.gstate.ctm,
            ..Camera::default()
        };

        let mut params = self.parse_param_list()?;

        // TODO: not great parsing

        let typ = params.expect_simple()?;
        match typ {
            "orthographic" => cam.typ = CameraTyp::Orthographic,
            "perspective" => cam.typ = CameraTyp::Perspective,
            "realistic" => cam.typ = CameraTyp::Realistic,
            "spherical" => cam.typ = CameraTyp::Spherical,
            cam => return Err(eyre!("Unkown camera type: '{}'", cam)),
        };

        for p in params.params() {
            match (p.name, &p.value) {
                ("fov", ListParamValue::Single(Value::Float(fov))) => {
                    cam.fov = *fov;
                }
                p => return Err(eyre!("Wrong Camera parameter: '{:?}'", p)),
            }
        }

        Ok(cam)
    }

    fn parse_sampler(&mut self) -> Result<()> {
        let _params = self.parse_param_list()?;

        /*
        match self.str(delim) {
            "halton" => self.parse_sampler_attribs(options, SamplerTyp::Halton)?,
            "independent" => todo!(),
            "paddedsobol" => todo!(),
            "sobol" => todo!(),
            "stratified" => todo!(),
            "zsobol" => todo!(),
            sampler => return Err(eyre!("Unkown sampler: '{}'", sampler)),
        };
        */

        Ok(())
    }

    fn parse_pixel_filter(&mut self) -> Result<()> {
        let _params = self.parse_param_list()?;

        /*
        match self.str(delim) {
            "box" => todo!(),
            "gaussian" => self.parse_pixel_filter_attribs(options, PixelFilterType::Gaussian)?,
            "mitchell" => todo!(),
            "sinc" => todo!(),
            "triangle" => todo!(),
            sampler => return Err(eyre!("Unkown pixel filter: '{}'", sampler)),
        };
        */

        Ok(())
    }

    fn parse_film(&mut self) -> Result<Film> {
        let mut film = Film::default();

        let params = self.parse_param_list()?;

        // TODO: not great parsing

        for p in params.params() {
            match (p.name, &p.value) {
                ("rgb", ListParamValue::Empty) => film.typ = FilmType::Rgb,
                ("gbuffer", ListParamValue::Empty) => todo!(),
                ("spectral", ListParamValue::Empty) => todo!(),
                ("filename", ListParamValue::Single(Value::String(filename))) => {
                    film.filename = String::from(*filename)
                }
                ("yresolution", ListParamValue::Single(Value::Integer(yres))) => {
                    film.yresolution = *yres
                }
                ("xresolution", ListParamValue::Single(Value::Integer(xres))) => {
                    film.xresolution = *xres
                }
                _ => return Err(eyre!("Unknown / unimplemented Film param: '{:?}'", p)),
            }
        }

        Ok(film)
    }

    fn parse_scene(&mut self) -> Result<(Vec<ShapeWithParams>, Option<InfiniteLightSource>)> {
        // let shape_attr = None;
        // let light_attr = None;
        // let material_attr = None;
        // let medium_attr = None;
        // let texture_attr = None;

        let mut shapes = Vec::new();
        let mut infinite_light = None;

        loop {
            if self.peek()? == &Lexeme::Eof {
                return Ok((shapes, infinite_light));
            }

            let name = self.expect(Lexeme::Str(""))?.unwrap_str();
            match name {
                "AttributeBegin" => self.saved_gstates.push(self.gstate.clone()),
                "AttributeEnd" => match self.saved_gstates.pop() {
                    Some(gstate) => self.gstate = gstate,
                    None => return Err(eyre!("Non-matching AttributeEnd directive")),
                },
                "Attribute" => {
                    todo!()
                    // "shape", "light", "material", "medium", or "texture"
                }
                "Shape" => {
                    let s = self.parse_shape()?;
                    shapes.push(s);
                }
                "ObjectBegin" => todo!(),
                "ObjectEnd" => todo!(),
                "LightSource" => {
                    let light = self.parse_light_source()?;
                    #[allow(irrefutable_let_patterns)]
                    if let LightSource::Infinite(ils) = light {
                        infinite_light = Some(ils);
                    }
                }
                "AreaLightSource" => self.parse_area_light_source()?,
                "Material" => self.parse_material_pre()?,
                "Texture" => self.parse_texture()?,
                // Materials
                "MakeNamedMaterial" => {
                    let (name, material) = self.parse_make_named_material()?;
                    self.materials.insert(name, material);
                }
                "NamedMaterial" => {
                    let name = self.parse_named_material()?;
                    self.gstate.material = Some(name);
                }
                // Mediums
                "MakeNamedMedium" => todo!(),
                "MediumInterface" => todo!(),
                // Transformations
                "Scale" => self.parse_scale()?,
                "Transform" => self.parse_transform()?,
                "ReverseOrientation" => {
                    let ori = &mut self.gstate.reverse_orientation;
                    *ori = !*ori;
                }
                // Invalid attributes
                opt @ ("Option" | "Camera" | "Samplesr" | "ColorSpace" | "Film" | "PixelFilter"
                | "Integrator" | "Accelerator" | "WorldBegin") => {
                    return Err(eyre!("Directive '{}' is invalid after WorldBegin", opt))
                }
                option => return Err(eyre!("Unkown option: '{}'", option)),
            };
        }
    }

    fn parse_shape(&mut self) -> Result<ShapeWithParams> {
        let mut params = self.parse_param_list()?;

        let typ = params.expect_simple()?;
        let shape = match typ {
            "bilinearmesh" => todo!(),
            "curve" => todo!(),
            "cylinder" => todo!(),
            "disk" => todo!(),
            "sphere" => Shape::Sphere(self.parse_sphere(&params)?),
            "trianglemesh" => Shape::TriMesh(self.parse_trianglemesh(&params)?),
            "plymesh" => Shape::TriMesh(self.parse_plymesh(&params)?),
            "loopsubdiv" => todo!(),
            t => return Err(eyre!("Inavalid Shape type: '{}'", t)),
        };

        // TODO: if materials and lights get large consider using something like Arc
        let material = if let Some(mat_name) = self.gstate.material {
            self.materials.get(mat_name).unwrap().clone()
        } else {
            Material::new_default(&self.rgbtospec)
        };

        Ok(ShapeWithParams::new(
            shape,
            material,
            self.gstate.area_light_source.clone(),
            self.gstate.ctm,
            self.gstate.reverse_orientation,
        ))
    }

    fn parse_sphere(&mut self, params: &ParamList) -> Result<Sphere> {
        let mut radius = 1.;

        for p in params.params() {
            match (p.name, &p.value) {
                ("radius", ListParamValue::Single(Value::Float(p_radius))) => radius = *p_radius,
                _ => return Err(eyre!("Unexpected sphere param: '{:?}'", p)),
            }
        }

        Ok(Sphere::new(radius))
    }

    fn parse_trianglemesh(&mut self, params: &ParamList) -> Result<TriMesh> {
        // TODO: be more robust when loading params ? Kinda annoying to do with this format...
        let mut indices: Option<Vec<i32>> = None;
        let mut points: Option<Vec<Vec3>> = None;
        let mut normals: Option<Vec<Vec3>> = None;
        let mut tangents: Option<Vec<Vec3>> = None;
        let mut uvs: Option<Vec<Vec2>> = None;

        for p in params.params() {
            match (p.name, &p.value) {
                ("indices", ListParamValue::List(ValueList::Integer(i))) => {
                    indices = Some(i.to_vec())
                }
                ("P", ListParamValue::List(ValueList::Point3(p))) => points = Some(p.to_vec()),
                ("N", ListParamValue::List(ValueList::Normal3(n))) => normals = Some(n.to_vec()),
                ("S", ListParamValue::List(ValueList::Vector3(t))) => tangents = Some(t.to_vec()),
                ("uv", ListParamValue::List(ValueList::Point2(uv))) => uvs = Some(uv.to_vec()),
                _ => return Err(eyre!("Unexpected triangle mesh param: '{:?}'", p)),
            }
        }

        let (indices, vertices) = match (indices, points) {
            (None, Some(vertices)) if vertices.len() == 3 => {
                let indices = vec![1, 2, 3];
                (indices, vertices)
            }
            (Some(indices), Some(vertices)) => (indices, vertices),
            _ => return Err(eyre!("Triangle mesh vertices or indices not specified")),
        };

        Ok(TriMesh::new(indices, vertices, normals, tangents, uvs))
    }

    fn parse_plymesh(&mut self, params: &ParamList) -> Result<TriMesh> {
        ply_mesh::parse_plymesh(&self.file_directory, params.params())
    }

    fn parse_light_source(&mut self) -> Result<LightSource> {
        let mut params = self.parse_param_list()?;

        let typ = params.next_param()?.expect_empty()?;

        let scale = if let Some(p) = params.get("scale") {
            p.expect_single()?.expect_float()?
        } else {
            1.0
        };

        match typ {
            "distant" => todo!(),
            "goniometric" => todo!(),
            "infinite" => {
                let filepath = if let Some(p) = params.get("filename") {
                    let filename = p.expect_single()?.expect_string()?;
                    self.file_directory.join(filename)
                } else {
                    todo!("infinite light source without texture");
                };

                if let Some(_) = params.get("illuminance") {
                    todo!("inifite light illuminance");
                }

                return Ok(LightSource::Infinite(InfiniteLightSource::new(
                    scale, filepath,
                )));
            }
            "point" => todo!(),
            "projection" => todo!(),
            "spot" => todo!(),
            _ => return Err(eyre!("Unknown LightSource type: '{}'", typ)),
        }
    }

    fn parse_area_light_source(&mut self) -> Result<()> {
        let mut params = self.parse_param_list()?;

        let typ = params.expect_simple()?;
        if typ != "diffuse" {
            return Err(eyre!("Unknown AreaLightSource type: '{:?}'", typ));
        }

        let color_space = self.gstate.color_space;
        let mut light = AreaLightSource::new_default(&self.rgbtospec, color_space);

        for p in params.params() {
            match (p.name, &p.value) {
                ("L", ListParamValue::Single(Value::Rgb(l))) => {
                    let spectrum = RgbSpectrum::new(
                        &self.rgbtospec,
                        *l,
                        RgbSpectrumKind::new_illuminant(color_space),
                    );
                    light.radiance = spectrum;
                }
                p => return Err(eyre!("Unknown AreaLightSourceParam: '{:?}'", p)),
            }
        }

        self.gstate.area_light_source = Some(light);
        Ok(())
    }

    fn parse_material_pre(&mut self) -> Result<()> {
        let _params = self.parse_param_list()?;

        eprintln!("Inline Materials aren't loaded properly yet");
        Ok(())
    }

    fn parse_material(&mut self, material_type: &str, mut params: ParamList) -> Result<Material> {
        let placeholder_material = || {
            eprintln!("Using a placeholder material");
            Ok(Material::new_default(&self.rgbtospec))
        };

        match material_type {
            "coateddiffuse" => return placeholder_material(),
            "coatedconductor" => return placeholder_material(),
            "conductor" => {
                let (vroughness, uroughness) = if let Some(p) = params.get("roughness") {
                    let r = p.expect_single()?.expect_float()?;
                    (r, r)
                } else if let (Some(vp), Some(up)) =
                    (params.get("vroughness"), params.get("uroughness"))
                {
                    let vr = vp.expect_single()?.expect_float()?;
                    let ur = up.expect_single()?.expect_float()?;
                    (vr, ur)
                } else {
                    (0., 0.)
                };

                let (ior, absorbtion_k) = if let Some(_) = params.get("reflectance") {
                    todo!()
                } else if let (Some(iorp), Some(absorbtionkp)) =
                    (params.get("k"), params.get("eta"))
                {
                    (
                        iorp.expect_single()?.expect_rgb()?,
                        absorbtionkp.expect_single()?.expect_rgb()?,
                    )
                } else {
                    return Err(eyre!(
                        "Neither reflectance nor k and eta found for conductor material"
                    ));
                };

                return Ok(Material::Conductor(ConductorMaterial::new(
                    self.rgbtospec,
                    ior,
                    absorbtion_k,
                    MaterialRoughness::new(vroughness, uroughness),
                )));
            }
            "dielectric" => return placeholder_material(),
            "diffuse" => {
                let reflectance = params
                    .next_param()?
                    .expect_single_named("reflectance")?
                    .expect_rgb()?;

                return Ok(Material::Diffuse(DiffuseMaterial::new(
                    &self.rgbtospec,
                    reflectance,
                )));
            }
            "diffusetransmission" => return placeholder_material(),
            "hair" => return placeholder_material(),
            "interface" => return placeholder_material(),
            "measured" => return placeholder_material(),
            "mix" => return placeholder_material(),
            "subsurface" => return placeholder_material(),
            "thindielectric" => return placeholder_material(),
            typ => return Err(eyre!("Unknown material type: '{}'", typ)),
        }
    }

    fn parse_make_named_material(&mut self) -> Result<(&'t str, Material)> {
        let mut params = self.parse_param_list()?;
        let name = params.expect_simple()?;

        let material_type = params
            .next_param()?
            .expect_single_named("type")?
            .expect_string()?;

        let material = self.parse_material(material_type, params)?;
        Ok((name, material))
    }

    fn parse_named_material(&mut self) -> Result<&'t str> {
        let mut params = self.parse_param_list()?;
        params.expect_simple()
    }

    fn parse_texture(&mut self) -> Result<()> {
        let _params = self.parse_param_list()?;
        eprintln!("Textures aren't loaded properly yet");
        Ok(())
    }

    fn parse_scale(&mut self) -> Result<()> {
        let s = self.parse_vec3()?;
        let trans = Mat4::from_scale(s);
        self.modify_ctm(trans);
        Ok(())
    }

    fn parse_transform(&mut self) -> Result<()> {
        let mut cols = [0f32; 16];

        self.expect(Lexeme::OpenBracket)?;
        for e in &mut cols {
            *e = self.parse_float()?;
        }
        self.expect(Lexeme::CloseBracket)?;

        // TODO: check if this is column-major or row-major !
        // Transform resets the CTM to the specified matrix.
        let trans = Mat4::from_cols_array(&cols);
        self.modify_ctm(trans);

        Ok(())
    }

    fn parse_look_at(&mut self) -> Result<()> {
        let eye = self.parse_vec3()?;
        let look = self.parse_vec3()?;
        let up = self.parse_vec3()?;

        let trans = vecmath::look_at(eye, look, up);
        self.modify_ctm(trans);

        Ok(())
    }

    fn parse_param_list(&mut self) -> Result<ParamList<'t>> {
        let mut params = SmallVec::new();

        while self.peek()? == &Lexeme::Qoutes {
            self.next()?;
            let type_or_param = self.expect(Lexeme::Str(""))?.unwrap_str();
            if self.peek()? == &Lexeme::Qoutes {
                self.next()?;

                let param = ListParam::new(type_or_param, ListParamValue::Empty);
                params.push(param);

                continue;
            }

            let name = self.expect(Lexeme::Str(""))?.unwrap_str();
            self.expect(Lexeme::Qoutes)?;

            let mut has_bracket = false;
            if self.peek()? == &Lexeme::OpenBracket {
                has_bracket = true;
                self.next()?;
            }

            let param = match self.parse_param_values(type_or_param, has_bracket)? {
                SingleValueOrList::Value(value) => {
                    ListParam::new(name, ListParamValue::Single(value))
                }
                SingleValueOrList::List(values) => {
                    ListParam::new(name, ListParamValue::List(values))
                }
            };
            params.push(param);

            if has_bracket {
                self.expect(Lexeme::CloseBracket)?;
            }
        }

        Ok(ParamList::new(params))
    }

    fn parse_param_values(
        &mut self,
        typ: &str,
        might_be_list: bool,
    ) -> Result<SingleValueOrList<'t>> {
        match typ {
            "integer" => self.parse_value_list(
                Self::parse_int,
                Value::Integer,
                ValueList::Integer,
                might_be_list,
            ),
            "float" => self.parse_value_list(
                Self::parse_float,
                Value::Float,
                ValueList::Float,
                might_be_list,
            ),
            "point2" => self.parse_value_list(
                Self::parse_vec2,
                Value::Point2,
                ValueList::Point2,
                might_be_list,
            ),
            "vector2" => self.parse_value_list(
                Self::parse_vec2,
                Value::Vector2,
                ValueList::Vector2,
                might_be_list,
            ),
            "point3" => self.parse_value_list(
                Self::parse_vec3,
                Value::Point3,
                ValueList::Point3,
                might_be_list,
            ),
            "vector3" => self.parse_value_list(
                Self::parse_vec3,
                Value::Vector3,
                ValueList::Vector3,
                might_be_list,
            ),
            // Some file just use "normal"...
            "normal" | "normal3" => self.parse_value_list(
                Self::parse_vec3,
                Value::Normal3,
                ValueList::Normal3,
                might_be_list,
            ),
            "spectrum" => {
                // TODO: can contain a filename

                todo!()
            }
            "rgb" => {
                let v = self.parse_vec3()?;
                Ok(SingleValueOrList::Value(Value::Rgb(v)))
            }
            "blackbody" => {
                let num = self.parse_int()?;
                Ok(SingleValueOrList::Value(Value::Blackbody(num)))
            }
            "bool" => {
                let s = self.expect(Lexeme::Str(""))?.unwrap_str();
                let b = match s {
                    "true" => true,
                    "false" => false,
                    s => return Err(eyre!("Invalid bool value: '{}'", s)),
                };

                Ok(SingleValueOrList::Value(Value::Bool(b)))
            }
            "string" => {
                let s = self.parse_quoted_string()?;
                Ok(SingleValueOrList::Value(Value::String(s)))
            }
            // Exclusive to materials
            "texture" => {
                let s = self.parse_quoted_string()?;
                Ok(SingleValueOrList::Value(Value::Texture(s)))
            }
            _ => Err(eyre!("Unknown type: '{}'", typ)),
        }
    }

    fn parse_value_list<T: Copy>(
        &mut self,
        parse: fn(&mut Self) -> Result<T>,
        wrap_value: fn(T) -> Value<'t>,
        wrap_list: fn(ValueVec<T>) -> ValueList,
        might_be_list: bool,
    ) -> Result<SingleValueOrList<'t>> {
        let mut valuevec = ValueVec::new();

        while self.peek()? != &Lexeme::CloseBracket {
            let ele = parse(self)?;
            valuevec.push(ele);

            if !might_be_list {
                break;
            }
        }

        if valuevec.len() > 1 {
            Ok(SingleValueOrList::List(wrap_list(valuevec)))
        } else if valuevec.len() == 1 {
            Ok(SingleValueOrList::Value(wrap_value(valuevec[0])))
        } else {
            Err(eyre!("No value supplied for parameter"))
        }
    }

    fn parse_int(&mut self) -> Result<Int> {
        let num = self.expect(Lexeme::Num(""))?.unwrap_num();
        let num = str::parse::<Int>(num)?;
        Ok(num)
    }

    fn parse_float(&mut self) -> Result<f32> {
        let num = self.expect(Lexeme::Num(""))?.unwrap_num();
        let num = str::parse::<f32>(num)?;
        Ok(num)
    }

    fn parse_vec2(&mut self) -> Result<Vec2> {
        let x = self.parse_float()?;
        let y = self.parse_float()?;
        Ok(Vec2::new(x, y))
    }

    fn parse_vec3(&mut self) -> Result<Vec3> {
        let x = self.parse_float()?;
        let y = self.parse_float()?;
        let z = self.parse_float()?;
        Ok(Vec3::new(x, y, z))
    }

    fn parse_quoted_string(&mut self) -> Result<&'t str> {
        self.expect(Lexeme::Qoutes)?;
        let s = self.expect(Lexeme::Str(""))?.unwrap_str();
        self.expect(Lexeme::Qoutes)?;
        Ok(s)
    }

    fn modify_ctm(&mut self, next_trans: Mat4) {
        let ctm = &mut self.gstate.ctm;
        *ctm = *ctm * next_trans;
    }

    fn peek(&mut self) -> Result<&Lexeme<'t>> {
        self.lexer.peek()
    }

    fn next(&mut self) -> Result<Lexeme<'t>> {
        self.lexer.next()
    }

    fn expect(&mut self, lex: Lexeme) -> Result<Lexeme<'t>> {
        let l = self.lexer.next()?;
        if std::mem::discriminant(&lex) == std::mem::discriminant(&l) {
            Ok(l)
        } else {
            Err(eyre!("Expected token '{:?}', got '{:?}'", lex, l))
        }
    }
}
