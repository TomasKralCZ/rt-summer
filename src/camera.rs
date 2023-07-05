use glam::{vec3, Vec2, Vec3};

use crate::geometry::Ray;

pub struct Camera {
    origin: Vec3,
    bottom_left: Vec3,
    viewport_width: f32,
    viewport_height: f32,
}

impl Camera {
    pub fn new(width: usize, height: usize, fov: f32) -> Self {
        let aspect_ratio = width as f32 / height as f32;

        let viewport_height = 2.;
        let viewport_width = viewport_height * aspect_ratio;

        // From PBRT docs about FOV:
        // This is the spread angle of the viewing frustum along the narrower of the image's width and height.
        let narrower = viewport_width.min(viewport_height);
        let focal_length = -(narrower / 2.) / f32::tan(fov.to_radians() / 2.);

        let origin = Vec3::ZERO;
        let horizontal = vec3(viewport_width, 0., 0.);
        let vertical = vec3(0., viewport_height, 0.);
        let bottom_left = origin - horizontal / 2. - vertical / 2. - vec3(0., 0., focal_length);

        Self {
            origin,
            bottom_left,
            viewport_width,
            viewport_height,
        }
    }

    pub fn gen_ray(&self, uv: Vec2) -> Ray {
        let offset = vec3(uv.x, uv.y, 0.) * vec3(self.viewport_width, self.viewport_height, 0.);

        let screencoord = self.bottom_left + offset;

        Ray::new(self.origin, screencoord - self.origin)
    }
}

#[cfg(test)]
mod test_camera {
    use super::*;

    #[test]
    fn test_cam_uv() {
        let cam = Camera::new(100, 100, 90.);
        assert_eq!(
            cam.gen_ray(Vec2::splat(0.5)),
            Ray::new(Vec3::ZERO, vec3(0., 0., 1.))
        );
    }
}
