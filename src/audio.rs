use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use ffmpeg_next::{codec::audio, frame::Audio};
use sdl3::{
    AudioSubsystem, Error, Sdl,
    audio::{AudioCallback, AudioSpec, AudioStreamWithCallback},
};

use crate::decode::MDecodeVideoFrame;

pub fn init_audio_subsystem<'a>(sdl: &Sdl, spec: &AudioSpec) -> Result<MPlayerAudio, Error> {
    let audio = sdl.audio()?;
    let (tx, rx) = mpsc::channel();
    let ctx = MPlayerAudioCallbackCtx::new(rx);
    let device: sdl3::audio::AudioStreamWithCallback<MPlayerAudioCallbackCtx> =
        audio.open_playback_stream(&spec, ctx).expect("");
    let _ = device.resume();
    let audio_sys = MPlayerAudio {
        tx,
        audio_spec: *spec,
        device: device,
    };

    Ok(audio_sys)
}

pub struct MPlayerAudio {
    pub tx: Sender<Option<Audio>>,
    pub audio_spec: AudioSpec,
    pub device: AudioStreamWithCallback<MPlayerAudioCallbackCtx>,
}

pub struct MPlayerAudioCallbackCtx {
    buffer: Arc<Mutex<Vec<u8>>>,
    thread: JoinHandle<()>,
}

impl MPlayerAudioCallbackCtx {
    pub fn new(audio_rx: Receiver<Option<Audio>>) -> MPlayerAudioCallbackCtx {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let thread_buffer_arc = Arc::clone(&buffer);
        let join_handle = thread::Builder::new()
            .name("audio".to_string())
            .spawn(move || {
                let buffer = thread_buffer_arc;
                for recv in audio_rx.iter() {
                    match recv {
                        Some(audio_frame) => {
                            let mut guard = buffer.lock().unwrap();

                            for byte in audio_frame.data(0) {
                                guard.push(*byte);
                            }
                        }
                        None => break,
                    }
                }
            })
            .unwrap();
        MPlayerAudioCallbackCtx {
            buffer,
            thread: join_handle,
        }
    }
}

impl AudioCallback<f32> for MPlayerAudioCallbackCtx {
    fn callback(&mut self, stream: &mut sdl3::audio::AudioStream, _: i32) {
        let mut buffer_lock = self.buffer.lock().unwrap();
        let _ = stream.put_data(&buffer_lock.as_slice());
        buffer_lock.clear();
    }
}
