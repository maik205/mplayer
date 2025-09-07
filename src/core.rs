use ffmpeg_next::{
    self as ffmpeg, Rational,
    codec::{Context, Parameters},
    format::Pixel,
    frame::{Audio, Video},
    software::scaling::Flags,
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, RwLock,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
};

use ffmpeg::{Error, Packet, Stream, format::input};

use crate::{
    constants::ConvFormat,
    decode::{MDecodeOptions, MediaInfo},
    utils::{calculate_tpf_from_time_base, height_from_ar, Range},
};

pub struct MPlayerCore {
    pub packet_queue: Arc<RwLock<VecDeque<(Packet, PacketMarker)>>>,
    pub look_range: Range,
    pub video: Option<DecodeThread<Video>>,
    pub audio: Option<DecodeThread<Audio>>,
    pub config: &'static MDecodeOptions,
    pub media_info: Option<MediaInfo>,
    pub officer_he_has_a_gun: Option<SyncSender<PacketDistributorCommand>>,
    pub what_gun: Option<JoinHandle<()>>,
    pub has_media: bool,
}

pub enum MediaThreadCommand {
    Play,
    Pause,
    Seek(i64),
    Exit,
}

pub enum MediaThreadStatus {
    Paused(u32),
    Seeking(/*From*/ u32, /*To*/ u32),
    Playing(u32),
    Stopped,
}
pub struct MediaThread {
    command_tx: Sender<MediaThreadCommand>,
    pub status: Arc<RwLock<MediaThreadStatus>>,
    handle: JoinHandle<()>,
}

pub struct StreamDescriptor {}

pub enum PacketDistributorCommand {
    Exit,
    MoveCursor(u32),
}

impl MPlayerCore {
    pub fn new(config: Option<&'static MDecodeOptions>) -> MPlayerCore {
        MPlayerCore {
            packet_queue: Arc::new(RwLock::new(VecDeque::new())),
            video: None,
            audio: None,
            config: config.unwrap_or_default(),
            media_info: None,
            has_media: false,
            officer_he_has_a_gun: None,
            what_gun: None,
            look_range: Range::new(10, 50),
        }
    }

    pub fn open_media(
        path: String,
        decode_options: Option<MDecodeOptions>,
        mutex: Arc<Mutex<MPlayerCore>>,
    ) -> MediaThread {
        let (command_tx, command_rx) = mpsc::channel::<MediaThreadCommand>();
        let status = Arc::new(RwLock::new(MediaThreadStatus::Paused(0)));
        let handle = thread::Builder::new()
            .name("main_media".to_string())
            .spawn(move || {
                if let Ok(mut input_ctx) = input(&path) {
                    let decode_options = decode_options.unwrap_or_default();

                    let mut video_tx = None;
                    let mut audio_tx = None;
                    let mut video_marker = None;
                    let mut audio_marker = None;
                    if let Some(video_stream) =
                        input_ctx.streams().best(ffmpeg_next::media::Type::Video)
                    {
                        let (p_tx_video, p_rx_video) = mpsc::sync_channel(100);
                        video_tx = Some(p_tx_video);
                        video_marker = Some(video_stream.convert());
                        if let Ok(mut lock) = mutex.lock() {
                            lock.video = Some(DecodeThread::<Video>::spawn(
                                video_stream.parameters(),
                                p_rx_video,
                                Some("video".to_string()),
                                Some(ThreadConfig {
                                    buffer_capacity: 10,
                                }),
                                Mutex::new(decode_options),
                            ));
                            lock.has_media = true;
                        }
                    }

                    if let Some(audio_stream) =
                        input_ctx.streams().best(ffmpeg_next::media::Type::Audio)
                    {
                        let (p_tx_audio, p_rx_audio) = mpsc::sync_channel(100);
                        audio_marker = Some(audio_stream.convert());
                        audio_tx = Some(p_tx_audio);
                        if let Ok(mut lock) = mutex.lock() {
                            lock.audio = Some(DecodeThread::<Audio>::spawn(
                                audio_stream.parameters(),
                                p_rx_audio,
                                Some("audio".to_string()),
                                Some(ThreadConfig {
                                    buffer_capacity: 10,
                                }),
                            ));
                            lock.has_media = true;
                        }
                    }

                    // Instead of collecting all packets, process them one by one to reduce memory pressure
                    let mut packet_iter = input_ctx.packets();

                    while let Some((stream, packet)) = packet_iter.next() {
                        if let Ok(command) = command_rx.try_recv() {
                            match command {
                                MediaThreadCommand::Play => {}
                                MediaThreadCommand::Pause => {
                                    if let Ok(MediaThreadCommand::Play) = command_rx.recv() {
                                        continue;
                                    }
                                }
                                MediaThreadCommand::Seek(to) => {
                                    // let _ = input_ctx.seek(to, ..);
                                }
                                MediaThreadCommand::Exit => {
                                    if let Some(ref mut vid_tx) = video_tx {
                                        let _ = vid_tx.send(ThreadData::Kill);
                                    }
                                    if let Some(ref mut audio_tx) = audio_tx {
                                        let _ = audio_tx.send(ThreadData::Kill);
                                    }
                                    break;
                                }
                            }
                        }
                        if let Some(marker) = &video_marker
                            && let Some(ref mut vid_tx) = video_tx
                        {
                            if marker.stream_index == stream.index() {
                                let _ = vid_tx.send(ThreadData::Packet(packet));
                                continue;
                            }
                        }
                        if let Some(marker) = &audio_marker
                            && let Some(ref mut aud_tx) = audio_tx
                        {
                            if marker.stream_index == stream.index() {
                                let _ = aud_tx.send(ThreadData::Packet(packet));
                                continue;
                            }
                        }
                    }
                }
            })
            .unwrap();
        MediaThread {
            command_tx,
            status: status.clone(),
            handle,
        }
    }
}

impl Default for ThreadConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 10,
        }
    }
}

pub struct PacketMarker {
    stream_index: usize,
    stream_id: i32,
}

impl ConvFormat<PacketMarker> for Stream<'_> {
    fn convert(&self) -> PacketMarker {
        PacketMarker {
            stream_index: self.index(),
            stream_id: self.id(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ThreadConfig {
    buffer_capacity: u16,
}

pub enum ThreadData {
    Packet(Packet),
    Kill,
}

pub struct DecodeThread<OutputType> {
    pub handle: JoinHandle<()>,
    pub output_rx: Receiver<OutputType>,
}

impl<T> DecodeThread<T> {
    pub fn join(self) {
        let _ = self.handle.join();
    }
}

#[derive(Clone, Copy)]
pub struct ScalingConfig {
    pub scaling_flag: Flags,
    pub width: u32,
    pub aspect_ratio: Rational,
    pub pixel_format: Pixel,
}

impl DecodeThread<Video> {
    pub fn spawn(
        parameters: Parameters,
        packet_rx: Receiver<ThreadData>,
        thread_name: Option<String>,
        config: Option<ThreadConfig>,
        scaling_config: Mutex<MDecodeOptions>,
    ) -> DecodeThread<Video> {
        let config = config.unwrap_or_default();
        let (output_tx, output_rx) = mpsc::sync_channel(config.buffer_capacity.into());

        let handle = thread::Builder::new()
            .name(thread_name.unwrap_or("media".to_string()))
            .spawn(move || {
                let mut video_decoder = Context::from_parameters(parameters)
                    .unwrap()
                    .decoder()
                    .video()
                    .unwrap();

                let scaling_config_;
                {
                    scaling_config_ = Some(scaling_config.lock().unwrap().clone());
                }
                let scaling_config = scaling_config_.unwrap();

                let mut scaling_context = ffmpeg::software::scaling::Context::get(
                    video_decoder.format(),
                    video_decoder.width(),
                    video_decoder.height(),
                    scaling_config.pixel_format,
                    scaling_config.window_default_size.0,
                    height_from_ar(
                        Rational(video_decoder.width() as i32, video_decoder.height() as i32),
                        scaling_config.window_default_size.0,
                    ),
                    scaling_config.scaling_flag,
                );

                let mut frame_buffer = Video::empty();
                let counter = 0;
                while let Ok(ThreadData::Packet(packet)) = packet_rx.recv() {
                    video_decoder.send_packet(&packet).unwrap();
                    if let Ok(_) = video_decoder.receive_frame(&mut frame_buffer) {
                        if let None = frame_buffer.pts() {
                            frame_buffer.set_pts(Some(counter * calculate_tpf_from_time_base(video_decoder.time_base(), video_decoder.frame_rate().unwrap())));
                        }                                
                        if let Ok(ref mut scaler) = scaling_context {
                            let mut output_buffer = Video::empty();
                            if let Ok(()) = scaler.run(&frame_buffer, &mut output_buffer) {
                                let _ = output_tx.send(output_buffer);
                                continue;
                            }
                        }
                        let _ = output_tx.send(frame_buffer.clone());
                    }
                }
            })
            .expect("Unable to spawn video thread, you may not have enough memory/cpu resource available.");
        DecodeThread {
            handle,
            output_rx: output_rx,
        }
    }
}

impl DecodeThread<Audio> {
    pub fn spawn(
        parameters: Parameters,
        packet_rx: Receiver<ThreadData>,
        thread_name: Option<String>,
        config: Option<ThreadConfig>,
    ) -> DecodeThread<Audio> {
        let config = config.unwrap_or_default();
        let (output_tx, output_rx) = mpsc::sync_channel(config.buffer_capacity.into());

        let handle = thread::Builder::new()
            .name(thread_name.unwrap_or("media".to_string()))
            .spawn(move || {
                let mut audio_decoder = Context::from_parameters(parameters)
                    .unwrap()
                    .decoder()
                    .audio()
                    .unwrap();
                let mut frame_buffer = Audio::empty();
                while let Ok(ThreadData::Packet(packet)) = packet_rx.recv() {
                    audio_decoder.send_packet(&packet).unwrap();
                    if let Ok(_) = audio_decoder.receive_frame(&mut frame_buffer) {
                        output_tx.send(frame_buffer.clone()).unwrap();
                    }
                }
            })
            .expect("Unable to spawn media thread.");
        DecodeThread {
            handle,
            output_rx: output_rx,
        }
    }
}

impl MediaThread {}
