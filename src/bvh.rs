use glam::Vec3;

use crate::{
    geometry::{Axis, Ray, AABB},
    scene::{primitive::Primitive, HitInfo},
    util::TaggedPtr,
};

// This BVH is basically taken straight out of PBRTv4 with small modifications
#[derive(Debug)]
pub struct Bvh {
    nodes: Vec<LinearBvhNode>,
}

impl Bvh {
    pub fn build(primitives: &mut [TaggedPtr<Primitive>]) -> Self {
        let mut bvh_primitives: Vec<BvhPrimitive> = primitives
            .iter()
            .enumerate()
            .map(|(i, prim)| BvhPrimitive::new(i, prim.aabb()))
            .collect();

        // Indices into primitives
        let mut ordered_primitives: Vec<usize> = Vec::with_capacity(bvh_primitives.len());
        let mut total_nodes = 0;

        let root = Self::build_recursive(
            &mut bvh_primitives,
            &mut ordered_primitives,
            &mut total_nodes,
        );

        drop(bvh_primitives);

        Self::sort_by_indices(primitives, ordered_primitives);

        let flattened = Self::flatten(&root, total_nodes);
        // FIXME: fix infinite loop in check_bvh
        //#[cfg(debug_assertions)]
        //flattened.check_bvh(&root, primitives);
        flattened
    }

    fn sort_by_indices<T>(data: &mut [T], mut indices: Vec<usize>) {
        for idx in 0..data.len() {
            if indices[idx] != idx {
                let mut current_idx = idx;
                loop {
                    let target_idx = indices[current_idx];
                    indices[current_idx] = current_idx;
                    if indices[target_idx] == target_idx {
                        break;
                    }
                    data.swap(current_idx, target_idx);
                    current_idx = target_idx;
                }
            }
        }
    }

    pub fn intersect(
        &self,
        ray: &Ray,
        mut tmax: f32,
        primitives: &[TaggedPtr<Primitive>],
    ) -> Option<HitInfo> {
        let inv_dir = Vec3::ONE / ray.dir;
        let dir_is_neg = inv_dir.cmplt(Vec3::ZERO);

        let mut current_node_index = 0;
        let mut to_visit_offset = 0;
        let mut nodes_to_visit = [0usize; 64];

        let mut closest_hitinfo = None;

        loop {
            let node = &self.nodes[current_node_index];
            if node.aabb.intersects(ray, tmax, inv_dir, dir_is_neg) {
                if node.primitive_count > 0 {
                    // Leaf node
                    let offset = node.primitive_offset_or_second_child_offset;
                    for prim_offset in offset..(offset + node.primitive_count as u32) {
                        let primitive = &primitives[prim_offset as usize];
                        if let Some(hitinfo) = primitive.intersect(ray) {
                            tmax = hitinfo.t;
                            closest_hitinfo = Some(hitinfo);
                        }
                    }

                    if to_visit_offset == 0 {
                        break;
                    } else {
                        to_visit_offset -= 1;
                        current_node_index = nodes_to_visit[to_visit_offset];
                    }
                } else {
                    // Interior node
                    let is_neg = match node.split_axis {
                        Axis::X => dir_is_neg.x,
                        Axis::Y => dir_is_neg.y,
                        Axis::Z => dir_is_neg.z,
                    };

                    // Negative axis optimization from PBRT
                    if is_neg {
                        nodes_to_visit[to_visit_offset] = current_node_index + 1;
                        to_visit_offset += 1;
                        current_node_index = node.primitive_offset_or_second_child_offset as usize;
                    } else {
                        nodes_to_visit[to_visit_offset] =
                            node.primitive_offset_or_second_child_offset as usize;
                        to_visit_offset += 1;
                        current_node_index += 1;
                    }
                }
            } else {
                if to_visit_offset == 0 {
                    break;
                } else {
                    to_visit_offset -= 1;
                    current_node_index = nodes_to_visit[to_visit_offset];
                }
            }
        }

        closest_hitinfo
    }

    fn flatten(root: &BuildBvhNode, total_nodes: usize) -> Self {
        let mut nodes = Vec::with_capacity(total_nodes);

        Self::flatten_inner(root, &mut nodes);

        Self { nodes }
    }

    fn flatten_inner(node: &BuildBvhNode, flat_nodes: &mut Vec<LinearBvhNode>) -> u32 {
        if node.primitive_count > 0 {
            // Leaf node
            debug_assert!(node.child_l.is_none() && node.child_r.is_none());
            debug_assert!(node.primitive_count <= u16::MAX as usize);
            debug_assert!(node.first_prim_offset <= u32::MAX as usize);

            let node = LinearBvhNode::new_leaf(
                node.aabb,
                node.first_prim_offset as u32,
                node.primitive_count as u16,
            );
            flat_nodes.push(node);
            1
        } else {
            // Interior node
            let linear_node = LinearBvhNode::new_interior(node.aabb, 0, node.split_axis);
            let index = flat_nodes.len();
            flat_nodes.push(linear_node);

            let mut children_count =
                Self::flatten_inner(node.child_l.as_ref().unwrap(), flat_nodes);

            flat_nodes[index].primitive_offset_or_second_child_offset =
                index as u32 + children_count + 1;

            children_count += Self::flatten_inner(node.child_r.as_ref().unwrap(), flat_nodes);

            children_count + 1
        }
    }

    /// Taken from PBRTv4
    fn build_recursive(
        bvh_primitives: &mut [BvhPrimitive],
        ordered_primitives: &mut Vec<usize>,
        total_nodes: &mut usize,
    ) -> BuildBvhNode {
        *total_nodes += 1;
        let aabb = bvh_primitives
            .iter()
            .fold(AABB::EMPTY, |bounds, p| bounds.union_aabb(p.aabb));

        let mut create_leaf_node = || {
            let first_prim_offset = ordered_primitives.len();
            for bvh_prim in &*bvh_primitives {
                ordered_primitives.push(bvh_prim.id);
            }
            BuildBvhNode::new_leaf(aabb, first_prim_offset, bvh_primitives.len())
        };

        if aabb.area() == 0. || bvh_primitives.len() == 1 {
            return create_leaf_node();
        } else {
            // Interior node
            let mid;
            let centroids_aabb = bvh_primitives.iter().fold(AABB::EMPTY, |bounds, prim| {
                bounds.union_point(prim.aabb.center())
            });

            let split_axis = centroids_aabb.max_axis();
            if centroids_aabb.is_empty() {
                return create_leaf_node();
            } else {
                if bvh_primitives.len() <= 2 {
                    mid = bvh_primitives.len() / 2;
                    // Equal-counts split method, applying the SAH here doesn't make sense
                    bvh_primitives.select_nth_unstable_by(mid, |p0, p1| {
                        p0.aabb.center()[split_axis as usize]
                            .partial_cmp(&p1.aabb.center()[split_axis as usize])
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                } else {
                    // Surface-area heuristic split method
                    let mut buckets = [BvhSahBucket::new_emnpty(); SAH_BUCKETS];
                    for prim in &*bvh_primitives {
                        let mut bucket = (SAH_BUCKETS as f32
                            * centroids_aabb.offset_of(prim.aabb.center())[split_axis as usize])
                            as usize;

                        if bucket == SAH_BUCKETS {
                            bucket -= 1;
                        }

                        buckets[bucket].count += 1;
                        buckets[bucket].aabb = buckets[bucket].aabb.union_aabb(prim.aabb);
                    }

                    const SPLIT_COUNT: usize = SAH_BUCKETS - 1;
                    let mut costs = [0.; SPLIT_COUNT];

                    let mut count_below = 0;
                    let mut aabb_below = AABB::EMPTY;
                    for i in 0..SPLIT_COUNT {
                        aabb_below = aabb_below.union_aabb(buckets[i].aabb);
                        count_below += buckets[i].count;
                        costs[i] += count_below as f32 * aabb_below.area();
                    }

                    let mut count_above = 0;
                    let mut aabb_above = AABB::EMPTY;
                    for i in (1..=SPLIT_COUNT).rev() {
                        aabb_above = aabb_above.union_aabb(buckets[i].aabb);
                        count_above += buckets[i].count;
                        costs[i - 1] += count_above as f32 * aabb_above.area();
                    }

                    let (min_cost_split_bucket, min_cost) = costs
                        .iter()
                        .enumerate()
                        .min_by(|(_, c0), (_, c1)| c0.total_cmp(c1))
                        .unwrap();

                    let min_cost = 0.5 + min_cost / aabb.area();
                    let leaf_cost = bvh_primitives.len();

                    if (bvh_primitives.len() > MAX_PRIMS_IN_NODE) || (min_cost < leaf_cost as f32) {
                        mid = bvh_primitives.iter_mut().partition_in_place(|prim| {
                            let mut bucket = (SAH_BUCKETS as f32
                                * centroids_aabb.offset_of(prim.aabb.center())[split_axis as usize])
                                as usize;

                            if bucket == SAH_BUCKETS {
                                bucket -= 1;
                            }

                            bucket <= min_cost_split_bucket
                        });

                        if mid == bvh_primitives.len() {
                            dbg!("shit");
                        }
                    } else {
                        return create_leaf_node();
                    }
                }
            }

            let child_l =
                Self::build_recursive(&mut bvh_primitives[..mid], ordered_primitives, total_nodes);
            let child_r =
                Self::build_recursive(&mut bvh_primitives[mid..], ordered_primitives, total_nodes);

            BuildBvhNode::new_interior(split_axis, child_l, child_r)
        }
    }

    /// Checks whether the flattened BVH is the same as the pointer-based BVH. Doesn't
    /// check other properties.
    fn check_flattened(&self, pointer_bvh: &BuildBvhNode) {
        let mut set = std::collections::BTreeSet::new();
        for offset in self
            .nodes
            .iter()
            .filter(|n| n.primitive_count == 0)
            .map(|n| n.primitive_offset_or_second_child_offset)
        {
            if !set.insert(offset) {
                panic!();
            }
        }

        let mut pointer_bvh_stack: Vec<&BuildBvhNode> = Vec::with_capacity(self.nodes.len());
        let mut flat_bvh_stack: Vec<usize> = Vec::with_capacity(self.nodes.len());

        pointer_bvh_stack.push(pointer_bvh);
        flat_bvh_stack.push(0);

        while !pointer_bvh_stack.is_empty() {
            let pointer_node = pointer_bvh_stack.pop().unwrap();
            let flat_index = flat_bvh_stack.pop().unwrap();
            let flat_node = &self.nodes[flat_index];

            // Compare nodes
            assert_eq!(pointer_node.aabb, flat_node.aabb);
            assert_eq!(pointer_node.split_axis, flat_node.split_axis);
            assert_eq!(
                pointer_node.primitive_count,
                flat_node.primitive_count as usize
            );

            if flat_node.primitive_count != 0 {
                // Comparing primitive offset only makes sense for leaf nodes
                assert_eq!(
                    pointer_node.first_prim_offset,
                    flat_node.primitive_offset_or_second_child_offset as usize
                );
            }

            // Traverse other nodes, traverse left sub-tree first
            match (&pointer_node.child_l, &pointer_node.child_r) {
                (None, None) => assert_ne!(pointer_node.primitive_count, 0),
                (Some(child_l), Some(child_r)) => {
                    pointer_bvh_stack.push(&child_r);
                    pointer_bvh_stack.push(&child_l);
                }
                _ => panic!(),
            }

            if flat_node.primitive_count == 0 {
                flat_bvh_stack.push(flat_node.primitive_offset_or_second_child_offset as usize);
                flat_bvh_stack.push(flat_index + 1);
            }
        }

        assert!(pointer_bvh_stack.is_empty());
        assert!(flat_bvh_stack.is_empty());
    }

    fn check_primitive_bounds(&self, primitives: &[TaggedPtr<Primitive>]) {
        let total_bounds = primitives
            .iter()
            .fold(AABB::EMPTY, |bounds, prim| bounds.union_aabb(prim.aabb()));
        assert!(total_bounds.fits_within(self.nodes[0].aabb));

        for (id, prim) in primitives.iter().enumerate() {
            let node = self
                .nodes
                .iter()
                .find(|node| {
                    let offset = node.primitive_offset_or_second_child_offset as usize;
                    let count = node.primitive_count as usize;

                    node.primitive_count > 0 && (offset <= id && (offset + count) > id)
                })
                .unwrap();

            let prim_aabb = prim.aabb();
            assert!(prim_aabb.fits_within(node.aabb));
        }
    }

    fn check_bvh(&self, root: &BuildBvhNode, primitives: &[TaggedPtr<Primitive>]) {
        self.check_flattened(&root);
        self.check_primitive_bounds(primitives);
    }
}

/// 32-byte alignment to make sure that a node doesn't cross into 2 cache lines
#[derive(Debug)]
#[repr(C, align(32))]
struct LinearBvhNode {
    aabb: AABB,
    primitive_offset_or_second_child_offset: u32,
    primitive_count: u16,
    split_axis: Axis,
}

impl LinearBvhNode {
    fn new_leaf(aabb: AABB, primitive_offset: u32, primitive_count: u16) -> Self {
        Self {
            aabb,
            primitive_offset_or_second_child_offset: primitive_offset,
            primitive_count,
            split_axis: Axis::X,
        }
    }

    fn new_interior(aabb: AABB, second_child_offset: u32, axis: Axis) -> Self {
        Self {
            aabb,
            primitive_offset_or_second_child_offset: second_child_offset,
            primitive_count: 0,
            split_axis: axis,
        }
    }
}

// TODO: make an enum out of this...
/// Pointer-based intermediate BVH
struct BuildBvhNode {
    aabb: AABB,
    split_axis: Axis,
    first_prim_offset: usize,
    primitive_count: usize,

    child_l: Option<Box<BuildBvhNode>>,
    child_r: Option<Box<BuildBvhNode>>,
}

impl BuildBvhNode {
    pub fn new_interior(split_axis: Axis, child_l: BuildBvhNode, child_r: BuildBvhNode) -> Self {
        Self {
            aabb: child_l.aabb.union_aabb(child_r.aabb),
            split_axis,
            first_prim_offset: 0,
            primitive_count: 0,
            child_l: Some(Box::new(child_l)),
            child_r: Some(Box::new(child_r)),
        }
    }

    pub fn new_leaf(aabb: AABB, first_prim_offset: usize, primitive_count: usize) -> Self {
        Self {
            aabb,
            split_axis: Axis::X,
            first_prim_offset,
            primitive_count,
            child_l: None,
            child_r: None,
        }
    }
}

impl std::fmt::Debug for BuildBvhNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildBVHNode")
            .field("aabb", &self.aabb)
            .field("child_l", &self.child_l)
            .field("child_r", &self.child_r)
            .finish()
    }
}

struct BvhPrimitive {
    /// Index to the primitive array
    id: usize,
    aabb: AABB,
}

impl BvhPrimitive {
    fn new(id: usize, aabb: AABB) -> Self {
        Self { id, aabb }
    }
}

const SAH_BUCKETS: usize = 12;
const MAX_PRIMS_IN_NODE: usize = 4;

#[derive(Clone, Copy)]
struct BvhSahBucket {
    count: u32,
    aabb: AABB,
}

impl BvhSahBucket {
    fn new_emnpty() -> Self {
        Self {
            count: 0,
            aabb: AABB::EMPTY,
        }
    }
}

#[cfg(test)]
mod test_super {
    use std::sync::Arc;

    use glam::{vec2, vec3};
    use rand::{distributions::Uniform, prelude::Distribution, rngs::SmallRng, SeedableRng};

    use crate::{
        camera::Camera,
        geometry::{sphere::Sphere, Shape},
        pbrt_loader::SceneLoader,
        scene::{primitive::SimplePrimtive, Scene},
    };

    use super::*;

    fn build_test_bvh() -> (Bvh, Vec<TaggedPtr<Primitive>>) {
        let sphere_0 = Sphere::new_mock(vec3(2., 0., 1.), 0.2);
        let sphere_1 = Sphere::new_mock(vec3(2., 0., -1.), 0.5);

        let sphere_2 = Sphere::new_mock(vec3(-2., 0., 1.), 0.1);
        let sphere_3 = Sphere::new_mock(vec3(-2., 0., -1.), 0.3);

        let material = Arc::new(crate::pbrt_loader::scene_description::Material::new_empty());

        let mut primitives: Vec<TaggedPtr<Primitive>> = [sphere_0, sphere_1, sphere_2, sphere_3]
            .into_iter()
            .map(|shape| {
                TaggedPtr::new(Primitive::Simple(Box::new(SimplePrimtive::new(
                    TaggedPtr::new(Shape::Sphere(Box::new(shape))),
                    material.clone(),
                ))))
            })
            .collect();

        (Bvh::build(&mut primitives), primitives)
    }

    #[test]
    fn test_bvh_build() {
        let (bvh, primitives) = build_test_bvh();

        // Interior nodes
        assert_eq!(
            bvh.nodes[0].aabb,
            AABB::new(vec3(-2.3, -0.5, -1.5), vec3(2.5, 0.5, 1.2))
        );
        assert_eq!(bvh.nodes[0].split_axis, Axis::X);
        assert_eq!(bvh.nodes[0].primitive_count, 0);

        assert_eq!(
            bvh.nodes[1].aabb,
            AABB::new(vec3(-2.3, -0.3, -1.3), vec3(-1.7, 0.3, 1.1))
        );
        assert_eq!(bvh.nodes[1].split_axis, Axis::Z);
        assert_eq!(bvh.nodes[1].primitive_count, 0);

        assert_eq!(
            bvh.nodes[4].aabb,
            AABB::new(vec3(1.5, -0.5, -1.5), vec3(2.5, 0.5, 1.2))
        );
        assert_eq!(bvh.nodes[4].split_axis, Axis::Z);
        assert_eq!(bvh.nodes[4].primitive_count, 0);

        // Leaf nodes
        assert_eq!(
            bvh.nodes[2].aabb,
            AABB::new(vec3(-2.3, -0.3, -1.3), vec3(-1.7, 0.3, -0.7))
        );
        assert_eq!(bvh.nodes[2].primitive_count, 1);
        let prim_index = bvh.nodes[2].primitive_offset_or_second_child_offset as usize;
        assert_eq!(primitives[prim_index].aabb(), bvh.nodes[2].aabb);

        assert_eq!(
            bvh.nodes[3].aabb,
            AABB::new(vec3(-2.1, -0.1, 0.9), vec3(-1.9, 0.1, 1.1))
        );
        assert_eq!(bvh.nodes[3].primitive_count, 1);
        let prim_index = bvh.nodes[3].primitive_offset_or_second_child_offset as usize;
        assert_eq!(primitives[prim_index].aabb(), bvh.nodes[3].aabb);

        assert_eq!(
            bvh.nodes[5].aabb,
            AABB::new(vec3(1.5, -0.5, -1.5), vec3(2.5, 0.5, -0.5))
        );
        assert_eq!(bvh.nodes[5].primitive_count, 1);
        let prim_index = bvh.nodes[5].primitive_offset_or_second_child_offset as usize;
        assert_eq!(primitives[prim_index].aabb(), bvh.nodes[5].aabb);

        assert_eq!(
            bvh.nodes[6].aabb,
            AABB::new(vec3(1.8, -0.2, 0.8), vec3(2.2, 0.2, 1.2))
        );
        assert_eq!(bvh.nodes[6].primitive_count, 1);
        let prim_index = bvh.nodes[6].primitive_offset_or_second_child_offset as usize;
        assert_eq!(primitives[prim_index].aabb(), bvh.nodes[6].aabb);
    }

    #[test]
    /// Tests that all intersections with the BVH match manual intersections.
    fn test_bvh_intersect() {
        let (bvh, primitives) = build_test_bvh();
        let mut rng = SmallRng::from_entropy();

        let rays = 100_000;
        let mut wrong = 0;

        for _ in 0..rays {
            // Create a ray facing the negative y axis
            let dist = Uniform::from(-0.2f32..0.2);
            let offset_x = dist.sample(&mut rng);
            let offset_z = dist.sample(&mut rng);
            let ray_orig = vec3(offset_x, 1., offset_z);

            let dist_x = Uniform::from(-2.5f32..2.7);
            let dist_y = Uniform::from(-0.7f32..0.7);
            let dist_z = Uniform::from(-1.7f32..1.4);
            let target_point = vec3(
                dist_x.sample(&mut rng),
                dist_y.sample(&mut rng),
                dist_z.sample(&mut rng),
            );
            let ray_dir = target_point - ray_orig;
            let ray = Ray::new(ray_orig, ray_dir);

            let bvh_closest_hit = bvh.intersect(&ray, f32::INFINITY, &primitives);

            let mut mint = f32::MAX;
            let mut manual_closest_hit = None;
            for prim in &primitives {
                if let Some(hit) = prim.intersect(&ray) {
                    if hit.t < mint {
                        mint = hit.t;
                        manual_closest_hit = Some(hit);
                    }
                }
            }

            match (bvh_closest_hit, manual_closest_hit) {
                (Some(bvh_hit), Some(manual_hit)) => {
                    assert_eq!(bvh_hit.pos, manual_hit.pos);
                    assert_eq!(bvh_hit.t, manual_hit.t);
                }
                (None, None) => (),
                (None, Some(_manual_hit)) => {
                    wrong += 1;
                }
                (Some(_bvh_hit), None) => {
                    // BVH should never produces false-positives
                    panic!();
                }
            }
        }

        println!(
            "Wrong rate: {}%, {} out of {}",
            (100. * wrong as f32) / rays as f32,
            wrong,
            rays
        );
        assert_eq!(wrong, 0);
    }

    #[test]
    fn test_bvh_intersect_sphere() {
        test_bvh_intersect_scene("resources/test/sphere.pbrt");
    }

    #[test]
    fn test_bvh_intersect_shortbox() {
        test_bvh_intersect_scene("resources/test/cornel-shortbox.pbrt");
    }

    /// Tests that all intersections with the BVH match manual intersections.
    fn test_bvh_intersect_scene(path: &str) {
        let scene_desc = SceneLoader::load_from_path(path).unwrap();
        let (width, height) = (
            scene_desc.options.film.xresolution,
            scene_desc.options.film.yresolution,
        );
        let cam = Camera::new(
            width as usize,
            height as usize,
            scene_desc.options.camera.fov,
        );

        let scene = Scene::init(scene_desc).unwrap();

        let mut rng = SmallRng::from_entropy();

        let rays = 100_000;
        let mut wrong = 0;

        for _ in 0..rays {
            let dist = Uniform::from(0f32..1f32);
            let uv = vec2(dist.sample(&mut rng), dist.sample(&mut rng));
            let ray = cam.gen_ray(uv);

            let bvh_closest_hit = scene.trace_ray(&ray);

            let mut mint = f32::MAX;
            let mut manual_closest_hit = None;
            for prim in scene.primitives() {
                if let Some(hit) = prim.intersect(&ray) {
                    if hit.t < mint {
                        mint = hit.t;
                        manual_closest_hit = Some(hit);
                    }
                }
            }

            match (bvh_closest_hit, manual_closest_hit) {
                (Some(bvh_hit), Some(manual_hit)) => {
                    assert_eq!(bvh_hit.pos, manual_hit.pos);
                    assert_eq!(bvh_hit.t, manual_hit.t);
                }
                (None, None) => (),
                (None, Some(_manual_hit)) => {
                    wrong += 1;
                }
                (Some(_bvh_hit), None) => {
                    // BVH should never produces false-positives
                    panic!();
                }
            }
        }

        println!(
            "Wrong rate: {}%, {} out of {}",
            (100. * wrong as f32) / rays as f32,
            wrong,
            rays
        );
        assert_eq!(wrong, 0);
    }
}
