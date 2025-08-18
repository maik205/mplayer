use std::{
    collections::{self, VecDeque},
    env,
    time::Instant,
};

use ffmpeg_next::{
    self as ffmpeg, Error,
    format::{Pixel, input},
    media::Type,
};
use sdl3::{EventPump, Sdl, VideoSubsystem, event::Event, render::Canvas, video::Window};

pub struct MPlayer {
    sdl_window: Window,
    sdl_video: VideoSubsystem,
    sdl_context: Sdl,
    initialized_at: Instant,
    sdl_event_pump: EventPump,
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
            });
        }
        Err(MPlayerError::WindowFailed)
    }
    //
    pub fn tick(&mut self) -> bool {
        for event in self.sdl_event_pump.poll_iter() {
            if let Event::Quit { .. } = event {
                return true;
            }
        }
        return false;
    }
}
pub struct MPlayerEvent {
    handler: Option<MEventHandler>,
    //Event src, event type, and others
}

pub enum MPlayerError {
    WindowFailed,
}

pub enum MPlayerShouldExit {
    True,
    False,
}

pub enum MPlayerEventType {}

pub type MEventHandler = Box<dyn Fn()>;
