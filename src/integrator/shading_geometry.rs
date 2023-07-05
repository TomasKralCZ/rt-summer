use glam::Vec3;

pub struct ShadingGeometry {
    pub cos_theta: f32,
    /// Halfway vector
    pub h: Vec3,
    pub noh: f32,
    pub nov: f32,
    pub hov: f32,
}

impl ShadingGeometry {
    pub fn new(normal: &Vec3, sample_dir: &Vec3, hit_ray_dir: &Vec3) -> Self {
        // FIXME: Hack when sample_dir and normal are parallel
        let cos_theta = normal.dot(*sample_dir).max(0.000001);
        let h = (*sample_dir - *hit_ray_dir).normalize();
        let noh = normal.dot(h);
        let nov = normal.dot(-*hit_ray_dir);
        let hov = h.dot(-*hit_ray_dir);

        Self {
            cos_theta,
            h,
            noh,
            nov,
            hov,
        }
    }
}
