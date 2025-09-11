use std::{
    collections::VecDeque,
    sync::{
        Arc, LazyLock, Mutex, RwLock,
        mpsc::{Receiver, channel},
    },
    thread,
    time::{Duration, Instant},
};

use ffmpeg_next::{
    Rational,
    frame::{Audio, Video},
    software::scaling::Flags,
};
use sdl3::{
    EventPump, Sdl, VideoSubsystem,
    audio::{AudioFormat, AudioSpec},
    event::Event,
    pixels::{Color, PixelFormatEnum},
    render::{Canvas, Texture},
    video::Window,
};

use crate::{
    Command,
    audio::init_audio_subsystem,
    constants::ConvFormat,
    core::MPlayerCore,
    utils::{MDecodeOptions, Range, convert_pts, time_base_to_ns},
};
use crate::{audio::MPlayerAudio, utils::calculate_wait_from_rational};

pub struct MPlayer {
    sdl: Sdl,
    _sdl_video: VideoSubsystem,
    _initialized_at: Instant,
    sdl_event_pump: EventPump,
    pub should_exit: bool,
    pub core: Arc<Mutex<MPlayerCore>>,
    canvas: Canvas<Window>,
    video_texture: Texture,
    // will use in future to display some player stats like yt's stats for nerds
    _player_stats: MPlayerStats,
    clock: u128,
    audio: Option<MPlayerAudio>,
    // Heartbeat
    beat: Instant,
    pub player_frequency: i32,
    internal_buff_v: Option<VecDeque<Video>>,
    internal_buff_a: Option<VecDeque<Audio>>,
}

pub struct MPlayerStats {
    time_to_present: f32,
    frame_count: u16,
    frame_count_instant: Instant,
}

const WINDOW_WIDTH: u32 = 100;
const WINDOW_HEIGHT: u32 = 100;

pub static OPTS: MDecodeOptions = MDecodeOptions {
    scaling_flag: Flags::BILINEAR,
    look_range: Range { min: 10, max: 100 },
    window_default_size: (1920, 1080),
    pixel_format: ffmpeg_next::format::Pixel::RGB24,
};

impl MPlayer {
    pub fn setup() -> Result<Self, MPlayerError> {
        let sdl_ctx = sdl3::init().map_err(|_| MPlayerError::WindowCreationFailed)?;

        let sdl_video = sdl_ctx.video().map_err(|_| MPlayerError::SDLInitError)?;

        let window =
            sdl3::video::WindowBuilder::new(&sdl_video, "MPlayer", WINDOW_WIDTH, WINDOW_HEIGHT)
                .resizable()
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
            sdl: sdl_ctx,
            _sdl_video: sdl_video,
            sdl_event_pump,
            _initialized_at: Instant::now(),
            should_exit: false,
            core,
            canvas,
            video_texture,
            _player_stats: MPlayerStats {
                time_to_present: -1.0,
                frame_count: 0,
                frame_count_instant: Instant::now(),
            },
            clock: 0,
            audio: None,
            beat: Instant::now(),
            player_frequency: 10000,
            internal_buff_a: None,
            internal_buff_v: None,
        })
    }
    pub fn tick(&mut self, cli_command: Option<Command>) -> () {
        if let Some(command) = cli_command {
            self.process_command(command);
        }

        // Check if there is an active decoder and obtains the frame
        if let Ok(lock) = &self.core.lock() {
            if let Some(video) = &lock.video {
                let hasnt_ticket_for = self.beat.elapsed();
                if hasnt_ticket_for.as_nanos() > time_base_to_ns(Rational(1, self.player_frequency))
                {
                    self.clock += hasnt_ticket_for.as_nanos()
                        / time_base_to_ns(Rational(1, self.player_frequency));
                    self.beat = Instant::now();
                    // println!("{}", self.clock);
                }

                if let Some(ref mut buff) = self.internal_buff_v {
                    if buff.len() < 10 {
                        if let Ok(frame) = video.output_rx.try_recv() {
                            buff.push_back(frame);
                        }
                    } else {
                        // println!("buffer cap reached");
                    }
                } else {
                    self.internal_buff_v = Some(VecDeque::new());
                }
                if let Some(ref mut buff) = self.internal_buff_v
                    && let Some(frame) = buff.front()
                    && let Some(pts) = frame.pts()
                {
                    if pts == 0 {
                        self.clock = 0;
                    }
                    if (convert_pts(
                        pts,
                        video.stream_info.time_base,
                        Rational(1, self.player_frequency),
                    ) as u128)
                        <= self.clock
                        && let Some(ref mut frame) = buff.pop_front()
                    {
                        self._player_stats.frame_count += 1;
                        if self
                            ._player_stats
                            .frame_count_instant
                            .elapsed()
                            .as_secs_f64()
                            > 1.0
                        {
                            println!("fps: {}", self._player_stats.frame_count);
                            self._player_stats.frame_count_instant = Instant::now();
                            self._player_stats.frame_count = 0;
                        }
                        println!(
                            "tick: stream_pts {} player_timer {}",
                            convert_pts(
                                pts,
                                video.stream_info.time_base,
                                Rational(1, self.player_frequency),
                            ),
                            self.clock
                        );
                        // println!("[video] {}", frame.pts().unwrap());
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
                        let _ = self.video_texture.with_lock(
                            None,
                            |buffer: &mut [u8], _pitch: usize| {
                                let frame_data = frame.data_mut(0);
                                buffer.swap_with_slice(frame_data);
                            },
                        );

                        self.canvas.clear();
                        let _ = self.canvas.copy(&self.video_texture, None, None);
                        self.canvas.present();
                    }
                }
            }

            if let Some(thread) = &lock.audio {
                if let Ok(frame) = thread.output_rx.try_recv() {
                    // println!("[audio] {}", audio.pts().unwrap());
                    if let Some(audio) = &self.audio {
                        let _ = audio.tx.send(frame);
                    } else {
                        self.audio = Some(
                            init_audio_subsystem(&self.sdl, thread.stream_info.spec.unwrap())
                                .unwrap(),
                        );
                    }
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
                self.beat = Instant::now();
            }
            _ => {}
        }
    }

    pub fn go(&mut self, commander: Receiver<Command>, tps: i32) {
        let (tick_tx, tick_rx) = channel::<()>();
        self.player_frequency = tps;
        let timer_t = thread::spawn(move || {
            // Move the commander into the thread

            loop {
                let _ = tick_tx.send(());
                thread::sleep(Duration::from_nanos(calculate_wait_from_rational(
                    Rational(1, tps),
                    crate::utils::TimeScale::Nano,
                )));
            }

            // Tick tx will get dropped, closing the channel and killing threads
        });
        let mut tick_count = 0;
        let mut timer = Instant::now();
        while let Ok(_) = tick_rx.recv() {
            if let Ok(command) = commander.try_recv() {
                self.process_command(command);
            }
            self.tick(None);
            tick_count += 1;
            if timer.elapsed().as_secs_f32() > 2.0 {
                println!(
                    "[TPS] {}",
                    tick_count as f32 / timer.elapsed().as_secs_f32()
                );
                tick_count = 0;
                timer = Instant::now();
            }
        }
        let _ = timer_t.join();
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
