use glam::{vec3, Mat4, Vec3, Vec4};

use crate::math::sqr;

/// Taken from: Building an Orthonormal Basis, Revisited
/// Tom Duff, James Burgess, Per Christensen, Christophe Hery, Andrew Kensler, Max Liani, and Ryusuke Villemin
pub fn coordinate_system(v1: Vec3) -> (Vec3, Vec3, Vec3) {
    let sign = f32::copysign(1., v1.z);
    let a = -1. / (sign + v1.z);
    let b = v1.x * v1.y * a;

    let v2 = vec3(1. + sign * sqr(v1.x) * a, sign * b, -sign * v1.x);
    let v3 = vec3(b, sign + sqr(v1.y) * a, -v1.y);

    (v1, v2, v3)
}

/// From PBRTv4 - RotateFromTo.
/// from and to have to be normalized...
pub fn rotate_from_to(from: Vec3, to: Vec3) -> Mat4 {
    // Compute intermediate vector for vector reflection
    let refl = if from.x < 0.72 && to.x.abs() < 0.72 {
        vec3(1., 0., 0.)
    } else if from.y < 0.72 && to.y.abs() < 0.72 {
        vec3(0., 1., 0.)
    } else {
        vec3(0., 0., 1.)
    };

    // Initialize matrix _r_ for rotation
    let u = refl - from;
    let v = refl - to;

    let mut arr = [[0f32; 4]; 4];

    for i in 0..3 {
        for j in 0..3 {
            let odd = if i == j { 1f32 } else { 0f32 };

            arr[i][j] = odd - 2. / u.dot(u) * u[i] * u[j] - 2. / v.dot(v) * v[i] * v[j]
                + 4. * u.dot(v) / (u.dot(u) * v.dot(v)) * v[i] * u[j];
        }
    }

    Mat4::from_cols_array_2d(&arr).transpose()
}

pub fn orient_dir(dir: Vec3, normal: Vec3) -> Vec3 {
    /* let up = if normal.z < 0.999 {
        vec3(0., 0., 1.)
    } else {
        vec3(1., 0., 0.)
    };

    let tangent = up.cross(normal).normalize();
    let bitangent = normal.cross(tangent);

    let mut sample_dir = tangent * dir.x + bitangent * dir.y + normal * dir.z; */

    let (_, b1, b2) = coordinate_system(normal);
    let mut sample_dir = b1 * dir.x + b2 * dir.y + normal * dir.z;

    sample_dir = sample_dir.normalize();

    if normal.dot(sample_dir) < 0. {
        // FIXME: it's usually really close to 0, unsure what to do here...
        sample_dir = -sample_dir;
    }

    sample_dir
}

pub fn look_at(eye: Vec3, look: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_lh(eye, look, up)
}

/// In a left-handed coordinate system with Y-up
pub fn spherical_to_cartesian(theta: f32, phi: f32) -> Vec3 {
    let x = theta.sin() * phi.cos();
    let y = theta.cos();
    let z = theta.sin() * phi.sin();
    vec3(x, y, z).normalize()
}

// TODO: calculate epsilon automatically
pub fn vec4_cmp_assert(a: Vec4, b: Vec4) {
    assert!(a.abs_diff_eq(b, 0.0001));
}

pub fn vec3_cmp_assert(a: Vec3, b: Vec3) {
    assert!(a.abs_diff_eq(b, 0.0001));
}

#[cfg(test)]
mod test_super {
    use glam::{vec4, Vec4};

    use super::*;

    #[test]
    fn test_rotate_from_to_same() {
        let from = vec3(0., 0., 1.);
        let to = vec3(0., 0., 1.);
        let trans = rotate_from_to(from, to);
        assert_eq!(
            trans,
            Mat4::from_cols(
                vec4(1., 0., 0., 0.),
                vec4(0., 1., 0., 0.),
                vec4(0., 0., 1., 0.),
                Vec4::ZERO
            )
        );
    }

    #[test]
    fn test_rotate_from_to_axis() {
        let from = vec3(1., 0., 0.);
        let to = vec3(0., 1., 0.);
        let trans = rotate_from_to(from, to);
        assert_eq!(trans * from.extend(0.), to.extend(0.));
    }

    #[test]
    fn test_rotate_from_to_arbitrary() {
        let from = vec3(0.5, 10., 4.).normalize();
        let to = vec3(3., 8., 11.).normalize();
        let trans = rotate_from_to(from, to);

        let res = trans * from.extend(0.);
        let correct = to.extend(0.);
        vec4_cmp_assert(res, correct);
    }

    #[test]
    fn test_spherical_to_cartesian() {
        // Upper hemisphere
        vec3_cmp_assert(
            spherical_to_cartesian(90f32.to_radians(), 0f32.to_radians()),
            vec3(1., 0., 0.),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(0f32.to_radians(), 0f32.to_radians()),
            vec3(0., 1., 0.),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(90f32.to_radians(), 90f32.to_radians()),
            vec3(0., 0., 1.),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(45f32.to_radians(), 0f32.to_radians()),
            vec3(0.5, 0.5, 0.).normalize(),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(45f32.to_radians(), 90f32.to_radians()),
            vec3(0., 0.5, 0.5).normalize(),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(90f32.to_radians(), 45f32.to_radians()),
            vec3(0.5, 0., 0.5).normalize(),
        );

        // Lower hemisphere
        vec3_cmp_assert(
            spherical_to_cartesian(135f32.to_radians(), 0f32.to_radians()),
            vec3(0.5, -0.5, 0.).normalize(),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(135f32.to_radians(), 90f32.to_radians()),
            vec3(0., -0.5, 0.5).normalize(),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(135f32.to_radians(), 180f32.to_radians()),
            vec3(-0.5, -0.5, 0.).normalize(),
        );

        vec3_cmp_assert(
            spherical_to_cartesian(135f32.to_radians(), 270f32.to_radians()),
            vec3(0., -0.5, -0.5).normalize(),
        );
    }
}
