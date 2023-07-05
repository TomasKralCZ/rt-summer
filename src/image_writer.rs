use crate::{film, pbrt_loader::scene_description};
use eyre::Result;

pub struct ImageWriter {
    filepath: String,
    width: u64,
    height: u64,
}

impl ImageWriter {
    pub fn new(film: &scene_description::Film) -> Self {
        Self {
            filepath: film.filename.clone(),
            width: film.xresolution as u64,
            height: film.yresolution as u64,
        }
    }

    pub fn write_film(&self, film: &film::Film, samples: u32) -> Result<()> {
        use exr::prelude::*;

        let get_pixel = |pos: exr::math::Vec2<usize>| {
            let mut rgb = film.get_rgb(pos.x(), self.height as usize - pos.y() - 1);
            rgb /= samples as f32;
            (
                f16::from_f32(rgb.x),
                f16::from_f32(rgb.y),
                f16::from_f32(rgb.z),
            )
        };

        let channels = SpecificChannels::rgb(get_pixel);

        let image = Image::from_layer(Layer::new(
            (self.width as usize, self.height as usize),
            LayerAttributes::named("main-layer"),
            Encoding::FAST_LOSSLESS,
            channels,
        ));

        let mut filepath = self.filepath.clone();
        filepath.push_str(".exr");

        image.write().to_file(&filepath)?;

        Ok(())
    }
}
