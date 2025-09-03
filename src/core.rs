use ffmpeg_next::{
    self as ffmpeg,
    frame::{Audio, Video},
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{JoinHandle, Thread},
    time::Instant,
};

use ffmpeg::{Error, Packet, Stream, format::input};

use crate::decode::MediaInfo;

pub struct MPlayerCore<'a> {
    packet_queue: VecDeque<(Packet, Stream<'a>)>,
    video: Option<MediaThread<Video>>,
    audio: Option<MediaThread<Audio>>,
    config: &'static MPlayerConfig,
    media_info: Option<MediaInfo>,
}

pub struct MPlayerConfig {}

impl Default for MPlayerConfig {
    fn default() -> Self {
        Self {}
    }
}

impl MPlayerCore<'_> {
    pub fn new(config: &'static MPlayerConfig) -> MPlayerCore<'static> {
        MPlayerCore {
            packet_queue: VecDeque::new(),
            video: None,
            audio: None,
            config: config,
            media_info: None,
        }
    }

    pub fn open_media(&mut self, path: String) -> Result<(), Error> {
        let input_ctx = input(&path)?;

        if let Some(video_stream) = input_ctx.streams().best(ffmpeg_next::media::Type::Video) {
            
        }

        Ok(())
    }
}

pub trait Renderer {
    fn display(media_info: &MediaInfo, data: &[u8]);
}

impl<T> MediaThread<T>
where
    T: Send + Sync + 'static,
{
    pub fn spawn(
        thread_name: String,
        func: impl Fn(Packet, Arc<Mutex<VecDeque<T>>>) + Send + Sync + 'static,
        thread_config: Option<ThreadConfig>,
    ) -> MediaThread<T> {
        let buffer: Arc<Mutex<VecDeque<T>>> = Arc::new(Mutex::new(VecDeque::new()));
        let buffer_t = Arc::clone(&buffer);
        let (tx, rx) = mpsc::channel::<ThreadCommand>();

        let handle = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let buffer = buffer_t;
                let func = func;
                let rx = rx;
                let config = thread_config.clone().unwrap_or_default();
                let mut buffer_size = 0;
                {
                    buffer_size = buffer
                        .lock()
                        .expect("Unable to acquire buffer lock in media thread.")
                        .len();
                }
                let mut last_size_check = Instant::now();
                for cmd in rx.iter() {
                    match cmd {
                        ThreadCommand::Packet(packet) => {
                            // Block until the buffer is free again.
                            // This would create a lot of contention though?
                            // Lets cache the buffer size and only refetch it every 1 sec./
                            loop {
                                if buffer_size < config.buffer_capacity.into() {
                                    break;
                                }
                                if last_size_check.elapsed().as_secs() > 1 {
                                    buffer_size = buffer
                                        .lock()
                                        .expect("Unable to acquire buffer lock in media thread.")
                                        .len();
                                    last_size_check = Instant::now();
                                }
                            }

                            func(packet, Arc::clone(&buffer));
                        }
                        ThreadCommand::Flush => {
                            buffer
                                .lock()
                                .expect("Unable to acquire buffer lock in media thread.")
                                .clear();
                        }
                        ThreadCommand::Kill => {
                            return;
                        }
                    }
                }
            })
            .unwrap();
        MediaThread {
            handle,
            output_buffer: buffer,
            tx,
        }
    }
}

pub struct MediaThread<T> {
    handle: JoinHandle<()>,
    output_buffer: Arc<Mutex<VecDeque<T>>>,
    tx: Sender<ThreadCommand>,
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 10,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ThreadConfig {
    buffer_capacity: u16,
}

pub enum ThreadCommand {
    Packet(Packet),
    Flush,
    Kill,
}
