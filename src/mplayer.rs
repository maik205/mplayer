use std::{
    collections::{self, VecDeque},
    env,
    net::Shutdown,
    time::Instant,
};

use ffmpeg_next::{
    self as ffmpeg, Error,
    format::{Pixel, input},
    media::Type,
};
use sdl3::{EventPump, Sdl, VideoSubsystem, event::Event, render::Canvas, video::Window};

use crate::Command;

pub struct MPlayer {
    sdl_window: Window,
    sdl_video: VideoSubsystem,
    sdl_context: Sdl,
    initialized_at: Instant,
    sdl_event_pump: EventPump,
    pub should_exit: bool,
}
const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;

impl MPlayer {
    pub fn setup() -> Result<MPlayer, MPlayerError> {
        if let Ok(sdl_ctx) = sdl3::init()
            && let Ok(sdl_video) = sdl_ctx.video()
            && let Ok(window) =
                sdl3::video::WindowBuilder::new(&sdl_video, "SDL Test", WINDOW_WIDTH, WINDOW_HEIGHT)
                    .build()
            && let Ok(sdl_event_pump) = sdl_ctx.event_pump()
        {
            return Ok(MPlayer {
                sdl_window: window,
                sdl_video,
                sdl_context: sdl_ctx,
                sdl_event_pump,
                initialized_at: Instant::now(),
                should_exit: false,
            });
        }
        Err(MPlayerError::WindowCreationFailed)
    }
    //
    pub fn tick(&mut self, cli_command: Option<Command>) -> () {
        if let Some(command) = cli_command {
            self.process_command(command);
        }
        for event in self.sdl_event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.should_exit = true;
                }
                _ => {}
            }
        }
    }

    pub fn process_command(&mut self, command: Command) -> () {
        match command {
            Command::Shutdown => {
                self.should_exit = true;
            }
            _ => {}
        }
    }
}
pub struct MPlayerEvent {
    handler: Option<MEventHandler>,
}

pub enum MPlayerError {
    WindowCreationFailed,
}

pub enum MPlayerEventType {}

pub type MEventHandler = Box<dyn Fn()>;
