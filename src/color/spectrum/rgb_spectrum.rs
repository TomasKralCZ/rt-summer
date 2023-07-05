use std::sync::OnceLock;

use eyre::Result;
use glam::Vec3;
use rgb2spec::RGB2Spec;

use crate::color::color_space::ColorSpace;

use super::{
    DenselySampledSpectrum, SampledWavelengths, SpectralQuantity, CIE_D65, CIE_Y_INTEGRAL,
};

pub static RGBTOSPEC: OnceLock<RGB2Spec> = OnceLock::new();

pub fn init_rgbtospec() -> Result<()> {
    let rgbtospec = RGB2Spec::load("resources/srgb-to-spec-64")?;
    #[cfg(test)]
    let _ = RGBTOSPEC.set(rgbtospec);
    #[cfg(not(test))]
    RGBTOSPEC.set(rgbtospec).unwrap();
    Ok(())
}

#[derive(Clone, Debug)]
pub struct RgbSpectrum {
    sigmoid_coeff: [f32; 3],
    kind: RgbSpectrumKind,
    scale: f32,
}

impl RgbSpectrum {
    pub fn new(rgbtospec: &RGB2Spec, rgb: Vec3, kind: RgbSpectrumKind) -> Self {
        let (scale, rgb) = match kind {
            RgbSpectrumKind::Reflectance => (1., rgb),
            RgbSpectrumKind::Unbounded | RgbSpectrumKind::Illuminant(_) => {
                let max = rgb.max_element();
                let scale = 2. * max;
                let rgb = if scale != 0. { rgb / scale } else { Vec3::ZERO };
                (scale, rgb)
            }
        };

        let coeff = rgbtospec.fetch(rgb.to_array());
        Self {
            sigmoid_coeff: coeff,
            kind,
            scale,
        }
    }

    pub fn new_empty() -> Self {
        Self {
            sigmoid_coeff: [0., 0., 0.],
            kind: RgbSpectrumKind::Reflectance,
            scale: 0.,
        }
    }

    pub fn eval_single(&self, lambda: f32) -> f32 {
        let mut res = self.scale * rgb2spec::eval_precise(self.sigmoid_coeff, lambda);
        if let RgbSpectrumKind::Illuminant(illuminant) = &self.kind {
            // FIXME: HACK for normalizing standard illuminant values to have luminance of 1
            res *= illuminant.eval_single(lambda) * (CIE_Y_INTEGRAL / 10789.7637);
        }

        res
    }

    pub fn eval(&self, lambdas: &SampledWavelengths) -> SpectralQuantity {
        let mut vals = lambdas.lambdas;
        vals.iter_mut().for_each(|l| *l = self.eval_single(*l));

        SpectralQuantity::new(vals)
    }
}

#[derive(Clone)]
pub enum RgbSpectrumKind {
    Reflectance,
    Unbounded,
    Illuminant(DenselySampledSpectrum),
}

impl std::fmt::Debug for RgbSpectrumKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reflectance => write!(f, "Reflectance"),
            Self::Unbounded => write!(f, "Unbounded"),
            Self::Illuminant(_) => write!(f, "Illuminant"),
        }
    }
}

impl RgbSpectrumKind {
    pub fn new_illuminant(color_space: ColorSpace) -> Self {
        match color_space {
            ColorSpace::Srgb => Self::Illuminant(CIE_D65),
            _ => todo!(),
        }
    }
}

#[cfg(test)]
mod test_super {
    use std::ops::Range;

    use crate::color::spectrum::{LAMBDA_MAX, LAMBDA_MIN};

    use super::*;

    #[test]
    fn test_rgbtospec_illuminant() {
        let rgb = [17., 12., 4.];
        let rgbtospec = RGB2Spec::load("resources/srgb-to-spec-64").unwrap();
        let rgbspectrum = RgbSpectrum::new(
            &rgbtospec,
            Vec3::from_array(rgb),
            RgbSpectrumKind::new_illuminant(ColorSpace::Srgb),
        );

        plot_spectrum(
            &rgbspectrum,
            "light-cornellbox",
            rgb,
            0f32..rgbspectrum.scale,
        );

        let rgb = [1., 1., 1.];
        let rgbspectrum = RgbSpectrum::new(
            &rgbtospec,
            Vec3::from_array(rgb),
            RgbSpectrumKind::new_illuminant(ColorSpace::Srgb),
        );

        plot_spectrum(&rgbspectrum, "light-one", rgb, 0f32..rgbspectrum.scale);
    }

    #[test]
    fn test_rgbtospec_reflectance() {
        let rgbtospec = RGB2Spec::load("resources/srgb-to-spec-64").unwrap();

        let red = Vec3::from_array([1., 0., 0.]);
        let r_rgbspectrum = RgbSpectrum::new(&rgbtospec, red, RgbSpectrumKind::Reflectance);
        plot_spectrum(&r_rgbspectrum, "reflectance-red", red.to_array(), 0f32..1.);

        let blue = Vec3::from_array([0., 0., 1.]);
        let b_rgbspectrum = RgbSpectrum::new(&rgbtospec, blue, RgbSpectrumKind::Reflectance);
        plot_spectrum(
            &b_rgbspectrum,
            "reflectance-blue",
            blue.to_array(),
            0f32..1.,
        );

        let green = Vec3::from_array([0., 1., 0.]);
        let g_rgbspectrum = RgbSpectrum::new(&rgbtospec, green, RgbSpectrumKind::Reflectance);
        plot_spectrum(
            &g_rgbspectrum,
            "reflectance-green",
            green.to_array(),
            0f32..1.,
        );

        let one = Vec3::from_array([0.9, 0.9, 0.9]);
        let one_rgbspectrum = RgbSpectrum::new(&rgbtospec, one, RgbSpectrumKind::Reflectance);
        plot_spectrum(
            &one_rgbspectrum,
            "reflectance-one",
            one.to_array(),
            0f32..1.,
        );

        let paper_one = Vec3::from_array([0.598, 0.305, 0.210]);
        let paper_one_rgbspectrum =
            RgbSpectrum::new(&rgbtospec, paper_one, RgbSpectrumKind::Reflectance);
        plot_spectrum(
            &paper_one_rgbspectrum,
            "reflectance-paper-one",
            paper_one.to_array(),
            0f32..1.,
        );

        let paper_three = Vec3::from_array([0.141, 0.855, 0.085]);
        let paper_three_rgbspectrum =
            RgbSpectrum::new(&rgbtospec, paper_three, RgbSpectrumKind::Reflectance);
        plot_spectrum(
            &paper_three_rgbspectrum,
            "reflectance-paper-three",
            paper_three.to_array(),
            0f32..1.,
        );

        let paper_six = Vec3::from_array([0.934, 0.951, 0.924]);
        let paper_six_rgbspectrum =
            RgbSpectrum::new(&rgbtospec, paper_six, RgbSpectrumKind::Reflectance);
        plot_spectrum(
            &paper_six_rgbspectrum,
            "reflectance-paper-six",
            paper_six.to_array(),
            0f32..1.,
        );
    }

    fn plot_spectrum(rgbspectrum: &RgbSpectrum, name: &str, color: [f32; 3], yrange: Range<f32>) {
        use plotters::prelude::*;

        std::fs::create_dir_all("./resources/test-results/rgb-to-spectrum/").unwrap();
        let name = format!("./resources/test-results/rgb-to-spectrum/{}.svg", name);
        let root_drawing_area = SVGBackend::new(&name, (1024, 384 * 2)).into_drawing_area();

        root_drawing_area.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(&root_drawing_area)
            .caption(
                format!("Spectrum for {:?} RGB: {:?}", rgbspectrum.kind, color),
                ("sans-serif", 30),
            )
            .set_label_area_size(LabelAreaPosition::Left, 40)
            .set_label_area_size(LabelAreaPosition::Bottom, 40)
            .build_cartesian_2d(LAMBDA_MIN..LAMBDA_MAX, yrange)
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            .label_style(("sans-serif", 20))
            .draw()
            .unwrap();

        let mut sum = color.iter().sum::<f32>();
        if sum < 1. {
            sum = 1.;
        }
        let r = ((color[0] / sum) * 255f32) as u8;
        let g = ((color[1] / sum) * 255f32) as u8;
        let b = ((color[2] / sum) * 255f32) as u8;

        let line_style = ShapeStyle {
            color: RGBAColor(r, g, b, 1.),
            filled: true,
            stroke_width: 2,
        };

        chart
            .draw_series(LineSeries::new(
                (LAMBDA_MIN..LAMBDA_MAX).map(|l| (l, rgbspectrum.eval_single(l as f32))),
                line_style,
            ))
            .unwrap();
    }
}
