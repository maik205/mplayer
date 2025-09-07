use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

use ffmpeg_next::{frame::Video, media, software::scaling::Flags};
use sdl3::{
    EventPump, Sdl, VideoSubsystem,
    audio::{AudioCallback, AudioFormat, AudioSpec},
    event::Event,
    pixels::{Color, PixelFormatEnum},
    render::{Canvas, Texture},
    video::Window,
};

use crate::{
    Command,
    constants::ConvFormat,
    core::MPlayerCore,
    decode::{DecoderCommand, MDecodeAudioFrame, MDecodeOptions, MDecodeVideoFrame, init},
    utils::Range,
};
use crate::{
    audio::MPlayerAudio,
    decode::{MDecode, MediaInfo},
};

pub struct MPlayer {
    _sdl_video: VideoSubsystem,
    _sdl_context: Sdl,
    _initialized_at: Instant,
    sdl_event_pump: EventPump,
    pub should_exit: bool,
    pub core: Arc<Mutex<MPlayerCore>>,
    canvas: Canvas<Window>,
    video_texture: Texture,
    // will use in future to display some player stats like yt's stats for nerds
    _player_stats: MPlayerStats,
    clock: u128,
    media_info: Option<MediaInfo>,
}

pub struct MPlayerStats {
    time_to_present: f32,
}

const WINDOW_WIDTH: u32 = 100;
const WINDOW_HEIGHT: u32 = 100;

pub static OPTS: MDecodeOptions = MDecodeOptions {
    scaling_flag: Flags::BILINEAR,
    look_range: Range { min: 10, max: 100 },
    window_default_size: (1920, 1080),
    pixel_format: ffmpeg_next::format::Pixel::RGB24,
    audio_spec: AudioSpec { freq: Some(22100), channels: Some(2), format: Some(AudioFormat::F32BE) }
};

impl MPlayer {
    pub fn setup() -> Result<Self, MPlayerError> {
        let sdl_ctx = sdl3::init().map_err(|_| MPlayerError::WindowCreationFailed)?;

        let sdl_video = sdl_ctx.video().map_err(|_| MPlayerError::SDLInitError)?;

        let window =
            sdl3::video::WindowBuilder::new(&sdl_video, "MPlayer", WINDOW_WIDTH, WINDOW_HEIGHT)
                .build()
                .map_err(|_| MPlayerError::WindowCreationFailed)?;

        let sdl_event_pump = sdl_ctx
            .event_pump()
            .map_err(|_| MPlayerError::EventPumpError)?;

        let mut canvas = window.into_canvas();
        canvas.set_draw_color(Color::RGB(100, 2, 0));

        let texture_creator = canvas.texture_creator();

        let video_texture = texture_creator
            .create_texture_streaming(
                Some(PixelFormatEnum::RGB24.into()),
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
            )
            .map_err(|_| MPlayerError::TextureCreationFailed)?;

        let core = Arc::new(Mutex::new(MPlayerCore::new(Some(&OPTS))));

        Ok(MPlayer {
            _sdl_video: sdl_video,
            _sdl_context: sdl_ctx,
            sdl_event_pump,
            _initialized_at: Instant::now(),
            should_exit: false,
            core,
            canvas,
            video_texture,
            _player_stats: MPlayerStats {
                time_to_present: -1.0,
            },
            clock: 0,
            media_info: None,
        })
    }
    pub fn tick(&mut self, cli_command: Option<Command>) -> () {
        if let Some(command) = cli_command {
            self.process_command(command);
        }

        // Check if there is an active decoder and obtains the frame
        if let Ok(lock) = &self.core.lock() {
            //check if enough time has passed since the image was last displayed and displays it if necessary

            if let Some(video) = &lock.video {
                if let Ok(ref mut frame) = video.output_rx.try_recv() {
                    /*
                    Sync code.. a bit later
                    while let Ok(frame) = video.output_rx.recv() &&
                    // We can safely unwrap since PTS is guaranteed to be assigned by the playback core.
                    frame.pts().unwrap() > 0
                    {}
                     */

                    let size = (frame.width(), frame.height());
                    if size != self.canvas.output_size().unwrap() {
                        let _ = self
                            .canvas
                            .window_mut()
                            .set_size(frame.width(), frame.height());
                        self.video_texture = self
                            .canvas
                            .texture_creator()
                            .create_texture_streaming(
                                Some(frame.format().convert().into()),
                                size.0,
                                size.1,
                            )
                            .unwrap();
                    }

                    let _ =
                        self.video_texture
                            .with_lock(None, |buffer: &mut [u8], _pitch: usize| {
                                let frame_data = frame.data_mut(0);
                                buffer.swap_with_slice(frame_data);
                            });

                    self.canvas.clear();
                    let _ = self.canvas.copy(&self.video_texture, None, None);
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

                std::process::exit(0);
            }
            Command::Play(path) => {
                MPlayerCore::open_media(path, Some(OPTS.clone()), Arc::clone(&self.core));
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
    DecoderInitFailed,
    DecoderOpenFailed,
    TextureCreationFailed,
    CanvasError,
    EventPumpError,
    MediaInfoUnavailable,
    SDLInitError,
    UnknownError,
}
