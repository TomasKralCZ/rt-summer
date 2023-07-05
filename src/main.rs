#![feature(array_chunks)]
#![feature(result_option_inspect)]
#![feature(float_next_up_down)]
#![feature(iter_partition_in_place)]
#![feature(allocator_api)]
#![allow(dead_code)]

use color::color_space::ColorSpace;
use integrator::Integrator;
use std::{sync::Arc, time::Duration, vec};

use camera::Camera;
use eyre::Result;
use image_writer::ImageWriter;
use lexopt::{
    Arg::{Long, Short},
    ValueExt,
};
use minifb::{Key, Window, WindowOptions};

use film::Film;
use render_threads::RenderContext;

use crate::scene::Scene;

mod bvh;
mod bxdf;
mod camera;
mod color;
mod film;
mod geometry;
mod image_writer;
mod integrator;
mod math;
mod pbrt_loader;
mod render_threads;
mod sampling;
mod scene;
mod texture;
mod util;
mod vecmath;

struct FrameBuffer {
    pub buffer: Vec<u32>,
}

impl FrameBuffer {
    fn new(w: usize, h: usize) -> Self {
        Self {
            buffer: vec![0; w * h],
        }
    }

    fn copy_from_film(&mut self, film: &Film, samples: u32) {
        for y in 0..film.height() {
            for x in 0..film.width() {
                let c = film.get_rgb(x, y);

                // Divide by the number of samples
                let c = c / samples as f32;

                // Tonemapping
                let c = c / (c + 1.);

                // Gamma correction
                const GAMMA: f32 = 2.2;
                let c = c.powf(1. / GAMMA);

                // Floating point to bytes
                let c = c.to_array().map(|f| (f * 255.0) as u8);
                self.buffer[film.width() * (film.height() - 1 - y) + x] =
                    u32::from_be_bytes([0, c[0], c[1], c[2]]);
            }
        }
    }
}

#[derive(Debug)]
pub struct CmdArgs {
    num_threads: usize,
    scene_path: String,
    integrator: String,
}

impl Default for CmdArgs {
    fn default() -> Self {
        Self {
            num_threads: num_cpus::get(),
            scene_path: "resources/scenes/cornell-box/scene-v4.pbrt".to_string(),
            integrator: "simple-path".to_string(),
        }
    }
}

fn parse_cmdargs() -> Result<CmdArgs> {
    let mut cmdargs = CmdArgs::default();

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Short('t') | Long("threads") => {
                cmdargs.num_threads = parser.value()?.parse()?;
            }
            Short('s') | Long("scene") => {
                cmdargs.scene_path = parser.value()?.parse()?;
            }
            Short('i') | Long("integrator") => {
                cmdargs.integrator = parser.value()?.parse()?;
            }
            _ => return Err(arg.unexpected().into()),
        }
    }

    Ok(cmdargs)
}

fn main() -> Result<()> {
    let cmdargs = parse_cmdargs()?;

    let scene_desc = pbrt_loader::SceneLoader::load_from_path(&cmdargs.scene_path)?;

    let image_writer = ImageWriter::new(&scene_desc.options.film);

    let (width, height) = (
        scene_desc.options.film.xresolution as usize,
        scene_desc.options.film.yresolution as usize,
    );

    let world_to_cam = scene_desc.options.camera.camera_from_world_transform;

    let mut framebuffer = FrameBuffer::new(width, height);
    let film = Film::new(width, height, ColorSpace::Srgb);
    let cam = Camera::new(width, height, scene_desc.options.camera.fov);
    // TODO: construct the Integrator based on the PBRT file input in the future
    let integrator = Integrator::new(&cmdargs.integrator)?;

    let scene = Scene::init(scene_desc)?;

    // TODO: think about if some of these should be stored in the integrator itself
    let render_context = Arc::new(RenderContext {
        cam,
        film,
        scene,
        integrator,
        camera_from_world: world_to_cam,
    });

    let mut threads =
        render_threads::RenderThreads::new(&cmdargs, width, height, render_context.clone())?;

    let mut window = Window::new(
        "Path tracing in one summer",
        width,
        height,
        WindowOptions::default(),
    )?;

    window.limit_update_rate(Some(std::time::Duration::from_secs(1)));

    let mut samples = 0;
    let mut update_screen = 1;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        util::timed_scope("1 sample render", || threads.render_once());

        //threads.render_once();

        samples += 1;
        println!("Samples: {samples}");

        if samples == update_screen {
            if update_screen >= 512 {
                update_screen += 256;
            } else {
                update_screen *= 2;
            }

            println!("Updating");
            image_writer.write_film(&render_context.film, samples)?;
            framebuffer.copy_from_film(&render_context.film, samples);
            window.update_with_buffer(&framebuffer.buffer, width, height)?;
        }

        if window.is_key_down(Key::P) {
            break;
        }
    }

    drop(threads);

    loop {
        std::thread::sleep(Duration::from_secs(15));
        window.update_with_buffer(&framebuffer.buffer, width, height)?;
    }
}
