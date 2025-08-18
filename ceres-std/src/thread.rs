use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::Relaxed;
use std::{
    fs::File,
    sync::{Mutex, atomic::AtomicU32},
};
use thread_priority::ThreadBuilderExt;

use {ceres_core::Pc1500, std::path::Path, std::sync::Arc};

pub trait PainterCallback: Send {
    fn paint(&self, pixel_data_rgba: &[u8]);
    fn request_repaint(&self);
}

pub struct Pc1500Thread {
    pc1500: Arc<Mutex<Pc1500>>,
    model: ceres_core::Model,
    exiting: Arc<AtomicBool>,
    pause_thread: Arc<AtomicBool>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    multiplier: Arc<AtomicU32>,
}

impl Pc1500Thread {
    pub fn new<P: PainterCallback + 'static>(
        model: ceres_core::Model,
        sav_path: Option<&Path>,
        rom_path: Option<&Path>,
        ctx: P,
    ) -> Result<Self, Error> {
        fn pc1500_loop<P: PainterCallback>(
            pc1500: &Arc<Mutex<Pc1500>>,
            exiting: &Arc<AtomicBool>,
            pause_thread: &Arc<AtomicBool>,
            multiplier: &Arc<AtomicU32>,
            ctx: &P,
        ) {
            let mut duration = ceres_core::FRAME_DURATION;

            let mut last_loop = std::time::Instant::now();

            // TODO: use condition variable

            while !exiting.load(Relaxed) {
                if !pause_thread.load(Relaxed) {
                    if let Ok(mut pc1500) = pc1500.lock() {
                        pc1500.step_frame();
                        ctx.paint(pc1500.pixel_data_rgba());
                    }
                    ctx.request_repaint();
                    duration = ceres_core::FRAME_DURATION / multiplier.load(Relaxed);
                }

                let elapsed = last_loop.elapsed();

                if elapsed < duration {
                    spin_sleep::sleep(duration - elapsed);
                }

                last_loop = std::time::Instant::now();
            }
        }

        let pc1500 = Self::create_new_pc1500(model)?;
        let pc1500 = Arc::new(Mutex::new(pc1500));

        let pause_thread = Arc::new(AtomicBool::new(false));

        let exiting = Arc::new(AtomicBool::new(false));

        let multiplier = Arc::new(AtomicU32::new(1));

        let thread_builder = std::thread::Builder::new().name("pc1500_loop".to_owned());
        let thread_handle = {
            let pc1500 = Arc::clone(&pc1500);
            let exit = Arc::clone(&exiting);
            let pause_thread = Arc::clone(&pause_thread);
            let multiplier = Arc::clone(&multiplier);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder.spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                pc1500_loop(&pc1500, &exit, &pause_thread, &multiplier, &ctx);
            })?
        };

        Ok(Self {
            pc1500,
            exiting,
            pause_thread,
            thread_handle: Some(thread_handle),
            model,
            multiplier,
        })
    }

    #[must_use]
    pub fn multiplier(&self) -> u32 {
        self.multiplier.load(Relaxed)
    }

    // Resets the GB state and loads the same ROM
    pub fn change_model(&mut self, model: ceres_core::Model) {
        if let Ok(mut gb) = self.pc1500.lock() {
            self.model = model;
            gb.change_model_and_soft_reset(model);
        }
    }

    fn create_new_pc1500(
        model: ceres_core::Model,
    ) -> Result<Pc1500, Error> {
        Ok(Pc1500::new(model))
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.pause_thread.load(Relaxed)
    }

    pub fn pause(&mut self) -> Result<(), Error> {
        self.pause_thread.store(true, Relaxed);
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), Error> {
        self.pause_thread.store(false, Relaxed);
        Ok(())
    }

    pub fn exit(&mut self) -> Result<(), Error> {
        self.exiting.store(true, Relaxed);
        self.thread_handle
            .take()
            .ok_or(Error::NoThreadRunning)?
            .join()
            .map_err(|_e| Error::ThreadJoin)?;
        Ok(())
    }

    pub fn press_release<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(&mut dyn Pressable) -> bool,
    {
        self.pc1500.lock().is_ok_and(|mut pc1500| f(&mut *pc1500))
    }

    #[must_use]
    pub const fn model(&self) -> ceres_core::Model {
        self.model
    }
}

impl Drop for Pc1500Thread {
    fn drop(&mut self) {
        if let Err(e) = self.exit() {
            eprintln!("error exiting pc1500_loop: {e}");
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    //Pc1500(ceres_core::Error),
    ThreadJoin,
    NoThreadRunning,
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "os error: {err}"),
            Self::ThreadJoin => write!(f, "thread join error"),
            Self::NoThreadRunning => write!(f, "no thread running"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

//TODO check && fix
pub trait Pressable {
    fn press(&mut self, button: ceres_core::Key);
    fn release(&mut self, button: ceres_core::Key);
}
//TODO check && fix
impl Pressable for Pc1500 {
    fn press(&mut self, button: ceres_core::Key) {
        self.press(button);
    }

    fn release(&mut self, button: ceres_core::Key) {
        self.release(button);
    }
}
