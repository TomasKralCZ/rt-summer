use glam::{vec3, Vec2, Vec3};

pub struct Texture {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    format: Format,
    wrap_u: WrapMode,
    wrap_v: WrapMode,
}

impl Texture {
    pub fn new(
        width: u32,
        height: u32,
        format: Format,
        pixels: &[u8],
        wrap_u: WrapMode,
        wrap_v: WrapMode,
    ) -> Self {
        let size = width * height * format.size();
        // This should be ok as per the GLTF spec
        assert_eq!(size as usize, pixels.len());

        Self {
            bytes: Vec::from(pixels),
            width,
            height,
            format,
            wrap_u,
            wrap_v,
        }
    }

    pub fn fetch_nearest(&self, uv: Vec2) -> Vec3 {
        let u = match self.wrap_u {
            WrapMode::Clamp => uv.x.clamp(0., 1.),
            WrapMode::Repeat => uv.x.rem_euclid(1.),
        };

        let v = match self.wrap_v {
            WrapMode::Clamp => uv.y.clamp(0., 1.),
            WrapMode::Repeat => uv.y.rem_euclid(1.),
        };

        let x = ((self.width - 1) as f32 * u) as usize;
        let y = ((self.height - 1) as f32 * v) as usize;

        let i = (x + (self.width as usize * y)) * self.format.size() as usize;

        match self.format {
            Format::R8 => {
                let r = self.bytes[i] as f32 / 255.;
                vec3(r, r, r)
            }
            Format::R8G8 => todo!(),
            Format::R8G8B8 => {
                let r = self.bytes[i] as f32 / 255.;
                let g = self.bytes[i + 1] as f32 / 255.;
                let b = self.bytes[i + 2] as f32 / 255.;

                vec3(r, g, b)
            }
            Format::R8G8B8A8 => {
                let r = self.bytes[i] as f32 / 255.;
                let g = self.bytes[i + 1] as f32 / 255.;
                let b = self.bytes[i + 2] as f32 / 255.;

                vec3(r, g, b)
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Format {
    R8,
    R8G8,
    R8G8B8,
    R8G8B8A8,
}

impl Format {
    pub fn size(self) -> u32 {
        match self {
            Format::R8 => 1,
            Format::R8G8 => 2,
            Format::R8G8B8 => 3,
            Format::R8G8B8A8 => 4,
        }
    }
}

#[derive(Clone, Copy)]
pub enum WrapMode {
    Clamp,
    Repeat,
}
