use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread,
};

use bus::{Bus, BusReader};
use eyre::Result;
use glam::{vec2, BVec3, DVec3, Mat4};
use rand::{distributions::Uniform, prelude::Distribution, rngs::SmallRng, SeedableRng};

use crate::{
    camera::Camera, color::spectrum::SampledWavelengths, film::Film, integrator::Integrator,
    scene::Scene, CmdArgs,
};

type ThreadId = usize;

pub struct RenderThreads {
    threads: Vec<thread::JoinHandle<()>>,
    render_state: Arc<FilmRenderState>,
    start_notify_bus: Bus<ThreadMsg>,
    completion_recv: Receiver<()>,
}

impl RenderThreads {
    pub fn new(
        cmdargs: &CmdArgs,
        width: usize,
        height: usize,
        render_context: Arc<RenderContext>,
    ) -> Result<Self> {
        let render_state = Arc::new(FilmRenderState::new(width, height));

        let mut threads = Vec::new();
        let (competion_send, completion_recv) = mpsc::sync_channel::<()>(cmdargs.num_threads);
        let mut start_notify_bus = Bus::new(cmdargs.num_threads);

        for thread_id in 0..cmdargs.num_threads {
            let render_state = Arc::clone(&render_state);
            let render_utils = render_context.clone();
            let start_rx = start_notify_bus.add_rx();
            let completion_send = competion_send.clone();

            threads.push(
                thread::Builder::new()
                    .name(thread_id.to_string())
                    .spawn(move || {
                        render(
                            thread_id,
                            start_rx,
                            render_state,
                            render_utils,
                            completion_send,
                        )
                    })?,
            );
        }

        Ok(Self {
            threads,
            render_state,
            start_notify_bus,
            completion_recv,
        })
    }

    pub fn render_once(&mut self) {
        self.render_state.reset();
        self.start_notify_bus.broadcast(ThreadMsg::Render);

        let mut completed_threads = 0;
        while completed_threads != self.threads.len() {
            self.completion_recv.recv().unwrap();
            completed_threads += 1;
        }
    }
}

#[derive(Clone)]
pub enum ThreadMsg {
    Render,
    Stop,
}

impl Drop for RenderThreads {
    fn drop(&mut self) {
        self.start_notify_bus.broadcast(ThreadMsg::Stop);

        let threads = std::mem::take(&mut self.threads);
        for t in threads {
            t.join().ok();
        }
    }
}

pub struct RenderContext {
    pub cam: Camera,
    pub film: Film,
    pub scene: Scene,
    pub integrator: Integrator,
    pub camera_from_world: Mat4,
}

const TILE_SIZE: usize = 8;

pub struct FilmRenderState {
    index: AtomicUsize,
    width: usize,
    height: usize,
    max_index: usize,
}

impl FilmRenderState {
    pub fn new(width: usize, height: usize) -> Self {
        let max_index = width * height - TILE_SIZE;

        Self {
            index: AtomicUsize::new(0),
            width,
            height,
            max_index,
        }
    }

    pub fn next_index(&self) -> Option<usize> {
        let index = self.index.fetch_add(TILE_SIZE, Ordering::Relaxed);
        if index >= self.max_index {
            None
        } else {
            Some(index)
        }
    }

    pub fn next_xy_coords(&self) -> Option<(usize, usize)> {
        self.next_index().map(|i| {
            let y = i / self.width;
            let x = i % self.width;
            debug_assert!(x < self.width);
            debug_assert!(y < self.height);
            (x, y)
        })
    }

    pub fn reset(&self) {
        self.index.store(0, Ordering::Relaxed);
    }
}

pub fn render(
    _thread_id: ThreadId,
    mut start_rx: BusReader<ThreadMsg>,
    render_state: Arc<FilmRenderState>,
    render_context: Arc<RenderContext>,
    completion_send: SyncSender<()>,
) {
    let mut rng = SmallRng::from_entropy();

    let (cam, film) = (&render_context.cam, &render_context.film);

    let mut sample = 0;

    loop {
        let msg = start_rx
            .recv()
            .expect("Master thread dropped, waiting for start");

        if let ThreadMsg::Stop = msg {
            return;
        }

        while let Some((px, py)) = render_state.next_xy_coords() {
            for px in px..(px + TILE_SIZE) {
                //----------------------------------------------------------------
                const STRATA_SQRT: usize = 4;
                let stratum_width = 1. / STRATA_SQRT as f32;

                let stratum = sample % (STRATA_SQRT * STRATA_SQRT);
                let stratum_offset_x = (stratum % STRATA_SQRT) as f32 * stratum_width;
                let stratum_offset_y = (stratum / STRATA_SQRT) as f32 * stratum_width;

                let dist = Uniform::from(0f32..stratum_width);
                let stratum_x = dist.sample(&mut rng);
                let stratum_y = dist.sample(&mut rng);

                let mut offset_x = stratum_offset_x + stratum_x;
                let mut offset_y = stratum_offset_y + stratum_y;
                offset_x = offset_x.clamp(0., 1f32.next_down());
                offset_y = offset_y.clamp(0., 1f32.next_down());
                //----------------------------------------------------------------

                let u = (offset_x + px as f32) / (render_state.width - 1) as f32;
                let v = (offset_y + py as f32) / (render_state.height - 1) as f32;

                let mut ray = cam.gen_ray(vec2(u, v));

                ray.transform(render_context.camera_from_world);
                let mut sampled_lambdas = SampledWavelengths::new_sample_uniform(&mut rng);

                let radiance = render_context.integrator.ray_l(
                    &ray,
                    &mut sampled_lambdas,
                    &render_context.scene,
                    &mut rng,
                );

                let xyz = sampled_lambdas.to_xyz(&radiance);

                assert!(xyz.cmpge(DVec3::ZERO) == BVec3::TRUE);
                assert!(!xyz.is_nan());

                unsafe {
                    // SAFETY: x, y coords are unique, we're good
                    film.accumulate(px, py, xyz);
                }
            }
        }

        sample += 1;

        completion_send
            .send(())
            .expect("Master thread dropped, sending completion message");
    }
}
