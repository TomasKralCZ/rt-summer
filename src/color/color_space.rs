use glam::{Mat3, Vec3};

#[derive(Clone, Copy)]
pub enum ColorSpace {
    Aces2065_1,
    Rec2020,
    DciP3,
    Srgb,
}

impl ColorSpace {
    /// Converts a color from XYZ to "self" color space.
    pub fn from_xyz(&self, xyz: Vec3) -> Vec3 {
        match self {
            ColorSpace::Aces2065_1 => todo!(),
            ColorSpace::Rec2020 => todo!(),
            ColorSpace::DciP3 => todo!(),
            ColorSpace::Srgb => (S_RGB_FROM_XYZ * xyz).clamp(Vec3::ZERO, Vec3::splat(f32::MAX)),
        }
    }
}

/// Taken from https://mina86.com/2019/srgb-xyz-matrix/.
/// Note that from_cols_array takes the matrix in a column order.
#[rustfmt::skip]
const S_RGB_FROM_XYZ: Mat3 = Mat3::from_cols_array(&[
    3.240812398895283,   -0.9692430170086407,  0.055638398436112804,
    -1.5373084456298136, 1.8759663029085742,   -0.20400746093241362,
    -0.4985865229069666, 0.04155503085668564,  1.0571295702861434,
]);
