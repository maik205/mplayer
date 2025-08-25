use std::time::Instant;

use ffmpeg_next::software::scaling::Flags;
use sdl3::{
    EventPump, Sdl, VideoSubsystem,
    event::Event,
    pixels::{Color, PixelFormat, PixelFormatEnum},
    render::{Canvas, FRect, Texture},
    video::Window,
};
use sdl3_sys::pixels::SDL_PixelFormat;

use crate::decode::MDecode;
use crate::{Command, decode::MDecodeOptions};

pub struct MPlayer {
    _sdl_video: VideoSubsystem,
    _sdl_context: Sdl,
    _initialized_at: Instant,
    sdl_event_pump: EventPump,
    pub should_exit: bool,
    pub decoder: Option<MDecode>,
    canvas: Canvas<Window>,
    video_texture: Texture,
    player_stats: MPlayerStats,
}
pub struct MPlayerStats {
    time_to_present: f32,
}

const WINDOW_WIDTH: u32 = 1920;
const WINDOW_HEIGHT: u32 = 1080;

impl MPlayer {
    pub fn setup() -> Result<Self, MPlayerError> {
        if let Ok(sdl_ctx) = sdl3::init()
            && let Ok(sdl_video) = sdl_ctx.video()
            && let Ok(window) =
                sdl3::video::WindowBuilder::new(&sdl_video, "MPlayer", WINDOW_WIDTH, WINDOW_HEIGHT)
                    .build()
            && let Ok(sdl_event_pump) = sdl_ctx.event_pump()
        {
            let mut canvas = window.into_canvas();
            canvas.set_draw_color(Color::RGB(100, 2, 0));

            let texture_creator = canvas.texture_creator();

            if let Ok(video_texture) = texture_creator.create_texture(
                Some(PixelFormatEnum::RGB24.into()),
                sdl3::render::TextureAccess::Streaming,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
            ) {
                return Ok(MPlayer {
                    _sdl_video: sdl_video,
                    _sdl_context: sdl_ctx,
                    sdl_event_pump,
                    _initialized_at: Instant::now(),
                    should_exit: false,
                    decoder: None,
                    canvas,
                    video_texture,
                    player_stats: MPlayerStats {
                        time_to_present: -1.0,
                    },
                });
            }
        }
        Err(MPlayerError::WindowCreationFailed)
    }
    //
    pub fn tick(&mut self, cli_command: Option<Command>) -> () {
        if let Some(command) = cli_command {
            self.process_command(command);
        }

        // Check if there is an active decoder and obtains the frame
        if let Some(decoder) = &mut self.decoder {
            let mut decoder = decoder;
            if let Some(mut frame) = decoder.next() {
                // Now we **finally** get to draw washoi!!
                // println!(
                //     "[DEBUG] {}s to decode frame",
                //     decoder.decoder_stats.time_to_frame
                // );
                let timer = Instant::now();
                let _ = self
                    .video_texture
                    .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                        let frame_data = frame.frame.data_mut(0);
                        buffer.swap_with_slice(frame_data);
                    });
                self.canvas.clear();
                self.player_stats.time_to_present = timer.elapsed().as_secs_f32();
                let _ = self.canvas.copy(&self.video_texture, None, None);
                self.canvas.present();
            } else {
                self.decoder.take();
                self.canvas.clear();
                self.canvas.present();
            }
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

    fn process_command(&mut self, command: Command) -> () {
        match command {
            Command::Shutdown => {
                self.should_exit = true;
            }
            Command::Play(path) => {
                if let Ok((w, h)) = self.canvas.output_size() {
                    if let Ok(decoder) = MDecode::open_video(
                        path.as_str(),
                        Some(MDecodeOptions {
                            scaling_flag: Flags::BILINEAR,
                            output_w: w,
                            output_h: h,
                            lookahead_count_video: 5,
                        }),
                    ) {
                        self.decoder = Some(decoder);
                    }
                }
            }
            _ => {}
        }
    }

    // I will handle resize later...
    // fn handle_resize(&mut self, event: WindowEvent) -> () {
    //     if let WindowEvent::Resized(w, h) = event {}
    // }
}

pub enum MPlayerError {
    WindowCreationFailed,
}
