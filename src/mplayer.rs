use std::time::Instant;

use ffmpeg_next::{Rational, software::scaling::Flags};
use sdl3::{
    EventPump, Sdl, VideoSubsystem,
    event::Event,
    pixels::{Color, PixelFormat, PixelFormatEnum},
    render::{Canvas, FRect, Texture},
    video::Window,
};
use sdl3_sys::pixels::SDL_PixelFormat;

use crate::{
    Command,
    decode::{MDecodeError, MDecodeOptions},
};
use crate::{decode::MDecode, utils::Range};

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
        let init_res = sdl3::init();
        if let Ok(sdl_ctx) = init_res {
            println!("[DEBUG] Context Initialization Ok");

            let sdl_video_res = sdl_ctx.video();
            if let Ok(sdl_video) = sdl_video_res {
                println!("[DEBUG] Video subsystem initialized");

                let window_res = sdl3::video::WindowBuilder::new(
                    &sdl_video,
                    "MPlayer",
                    WINDOW_WIDTH,
                    WINDOW_HEIGHT,
                )
                .build();

                if let Ok(window) = window_res {
                    println!("[DEBUG] Window created");

                    let event_pump_res = sdl_ctx.event_pump();
                    if let Ok(sdl_event_pump) = event_pump_res {
                        println!("[DEBUG] Event pump created");

                        let mut canvas = window.into_canvas();
                        canvas.set_draw_color(Color::RGB(100, 2, 0));

                        let texture_creator = canvas.texture_creator();

                        let texture_res = texture_creator.create_texture(
                            Some(PixelFormatEnum::RGB24.into()),
                            sdl3::render::TextureAccess::Streaming,
                            WINDOW_WIDTH,
                            WINDOW_HEIGHT,
                        );

                        if let Ok(video_texture) = texture_res {
                            println!("[DEBUG] Video texture created");
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
                        } else {
                            println!("[ERROR] Failed to create video texture");
                        }
                    } else {
                        println!("[ERROR] Failed to create event pump");
                    }
                } else {
                    println!("[ERROR] Failed to create window");
                }
            } else {
                println!("[ERROR] Failed to initialize video subsystem");
            }
        } else {
            if let Err(msg) = init_res {
                println!("{}", msg);
            }
        }
        Err(MPlayerError::WindowCreationFailed)
    }
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
                            look_range: Range::new(5, 15),
                            aspect_ratio: Rational::new(w as i32, h as i32),
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
#[derive(Debug)]
pub enum MPlayerError {
    WindowCreationFailed,
}
