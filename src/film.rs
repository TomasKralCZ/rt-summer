use std::cell::UnsafeCell;

use glam::{DVec3, Vec3};

use crate::color::color_space::ColorSpace;

pub struct Film {
    /// Stores XYZ values. Y = 0 is at the top.
    pub buffer: Box<[UnsafeCell<DVec3>]>,
    height: usize,
    width: usize,
    color_space: ColorSpace,
}

impl Film {
    pub fn new(width: usize, height: usize, color_space: ColorSpace) -> Self {
        let mut buffer = Vec::with_capacity(width * height);
        for _ in 0..(width * height) {
            buffer.push(UnsafeCell::new(DVec3::ZERO));
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            height,
            width,
            color_space,
        }
    }

    pub fn get_rgb(&self, x: usize, y: usize) -> Vec3 {
        let xyz = self.get_xyz(x, y);
        self.color_space.from_xyz(xyz.as_vec3())
    }

    fn get_xyz(&self, x: usize, y: usize) -> DVec3 {
        unsafe { *(self.buffer[self.width * y + x].get() as *const DVec3) }
    }

    /// This is unsafe because multiple threads writing to the same index is UB
    pub unsafe fn set(&self, x: usize, y: usize, val: DVec3) {
        let index = self.width * y + x;
        let ptr = self.buffer[index].get();
        ptr.write(val);
    }

    /// This is unsafe because multiple threads writing to the same index is UB
    pub unsafe fn accumulate(&self, x: usize, y: usize, val: DVec3) {
        let current = self.get_xyz(x, y);
        self.set(x, y, current + val);
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }
}

unsafe impl Sync for Film {}

#[cfg(test)]
mod test_film {
    use super::*;

    #[test]
    fn test_film_single_thread_miri() {
        let film = Film::new(16, 16, ColorSpace::Srgb);

        let a = &film;
        let b = &film;

        unsafe {
            a.set(0, 0, DVec3::ONE);
            b.set(0, 1, DVec3::ONE);
        }

        assert_eq!(film.get_xyz(0, 0), DVec3::ONE);
        assert_eq!(film.get_xyz(0, 1), DVec3::ONE);
    }

    #[test]
    fn test_film_multi_threaded_miri() {
        let film = Film::new(16, 16, ColorSpace::Srgb);

        let b = &film;
        let a = &film;

        std::thread::scope(|s| unsafe {
            s.spawn(|| {
                for _ in 0..1000 {
                    a.set(0, 0, DVec3::ONE);
                }
            });
            s.spawn(|| {
                for _ in 0..1000 {
                    b.set(0, 1, DVec3::ONE);
                }
            });
        });

        assert_eq!(film.get_xyz(0, 0), DVec3::ONE);
    }
}
