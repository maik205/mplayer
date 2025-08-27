use std::time::Instant;

use sdl3::{
    EventPump, Sdl, VideoSubsystem,
    event::Event,
    pixels::{Color, PixelFormatEnum},
    render::{Canvas, Texture},
    video::Window,
};

use crate::decode::MDecode;
use crate::{Command, decode::init};

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

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

impl MPlayer {
    pub fn setup() -> Result<Self, MPlayerError> {
        let init_res = sdl3::init();
        if let Ok(sdl_ctx) = init_res {
            println!("[DEBUG] Context Initialization Ok");

            let sdl_video_res = sdl_ctx.video();
            match sdl_video_res {
                Ok(sdl_video) => {
                    println!("[DEBUG] Video subsystem initialized");

                    let window_res = sdl3::video::WindowBuilder::new(
                        &sdl_video,
                        "MPlayer",
                        WINDOW_WIDTH,
                        WINDOW_HEIGHT,
                    )
                    .build();

                    match window_res {
                        Ok(window) => {
                            println!("[DEBUG] Window created");

                            let event_pump_res = sdl_ctx.event_pump();
                            match event_pump_res {
                                Ok(sdl_event_pump) => {
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

                                    match texture_res {
                                        Ok(video_texture) => {
                                            println!("[DEBUG] Video texture created");
                                            let decoder = Some(crate::decode::init(None));
                                            println!("[DEBUG] Decoder instantiated");

                                            return Ok(MPlayer {
                                                _sdl_video: sdl_video,
                                                _sdl_context: sdl_ctx,
                                                sdl_event_pump,
                                                _initialized_at: Instant::now(),
                                                should_exit: false,
                                                decoder,
                                                canvas,
                                                video_texture,
                                                player_stats: MPlayerStats {
                                                    time_to_present: -1.0,
                                                },
                                            });
                                        }
                                        Err(e) => {
                                            println!("[ERROR] Failed to create video texture: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("[ERROR] Failed to create event pump: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            println!("[ERROR] Failed to create window: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("[ERROR] Failed to initialize video subsystem: {e}");
                }
            }
        } else if let Err(e) = init_res {
            println!("[ERROR] Failed to initialize SDL context: {e}");
        }
        Err(MPlayerError::WindowCreationFailed)
    }
    pub fn tick(&mut self, cli_command: Option<Command>) -> () {
        if let Some(command) = cli_command {
            self.process_command(command);
        }

        // Check if there is an active decoder and obtains the frame
        if let Some(decoder) = &mut self.decoder {
            if decoder.is_active {
                let mut decoder = decoder;
                if let Some(mut frame) = decoder.next() {
                    let timer = Instant::now();
                    let size = (frame.frame.width(), frame.frame.height());
                    if size != self.canvas.output_size().unwrap() {
                        let _ = self
                            .canvas
                            .window_mut()
                            .set_size(frame.frame.width(), frame.frame.height());
                        self.video_texture = self
                            .canvas
                            .texture_creator()
                            .create_texture_streaming(
                                Some(PixelFormatEnum::RGB24.into()),
                                size.0,
                                size.1,
                            )
                            .unwrap();
                    }

                    let _ =
                        self.video_texture
                            .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                                let frame_data = frame.frame.data_mut(0);
                                buffer.swap_with_slice(frame_data);
                            });
                    self.canvas.clear();
                    self.player_stats.time_to_present = timer.elapsed().as_secs_f32();
                    let _ = self.canvas.copy(&self.video_texture, None, None);
                    self.canvas.present();
                } else {
                    self.canvas.clear();
                    self.canvas.present();
                }
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
                let _ = self.decoder.as_mut().unwrap().decoder_commander.send(None);
                std::process::exit(0);
            }
            Command::Play(path) => {
                self.decoder
                    .as_mut()
                    .unwrap()
                    .open_video(path.as_str(), None)
                    .unwrap();
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
