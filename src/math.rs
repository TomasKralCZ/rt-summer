use std::ops::{Add, Mul, Sub};

pub const EPS: f32 = 0.00001;

pub fn sqr<T>(val: T) -> T
where
    T: Clone + Copy + Mul<T, Output = T>,
{
    val * val
}

pub fn safe_sqrt(v: f32) -> f32 {
    // Sanity check
    if v < -EPS {
        panic!();
    }

    f32::sqrt(f32::max(0., v))
}

pub fn barycentric_interp<F, T>(bar: &[F; 3], e0: &T, e1: &T, e2: &T) -> T
where
    T: Add<T, Output = T>,
    T: Mul<F, Output = T>,
    F: Copy,
    T: Copy,
{
    (*e0 * bar[0]) + (*e1 * bar[1]) + (*e2 * bar[2])
}

/// t should be in the range [0..1)
pub fn lerp<T, S>(t: T, start: S, end: S) -> S
where
    S: Mul<T, Output = S>,
    S: Add<S, Output = S>,
    T: Sub<T, Output = T>,
    T: From<f32>,
    T: Copy,
{
    start * (Into::<T>::into(1f32) - t) + end * t
}
