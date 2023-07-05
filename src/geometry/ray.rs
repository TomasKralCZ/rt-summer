use glam::{Vec3, Mat4};

#[derive(Clone, PartialEq, Debug)]
pub struct Ray {
    pub orig: Vec3,
    pub dir: Vec3,
}

impl Ray {
    pub fn new(orig: Vec3, dir: Vec3) -> Self {
        Self {
            orig,
            dir: dir.normalize(),
        }
    }

    pub fn transform(&mut self, world_to_cam: Mat4) {
        self.dir = world_to_cam.inverse().transform_vector3(self.dir);
        self.orig = world_to_cam.inverse().transform_point3(self.orig);
    }
}

