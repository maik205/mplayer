use ffmpeg_next::{ Stream, codec::Parameters };
use sdl3::audio::AudioSpec;

use crate::{ constants::ConvFormat, core::StreamInfo };

impl ConvFormat<StreamInfo> for Stream<'_> {
    fn convert(&self) -> StreamInfo {
        let time_base = self.time_base();
        let codec = ffmpeg_next::codec::context::Context
            ::from_parameters(self.parameters())
            .unwrap();
        let kind = codec.medium();
        let mut fps = None;
        let mut spec = None;
        match codec.medium() {
            ffmpeg_next::media::Type::Video => {
                if let Ok(video) = codec.decoder().video() {
                    fps = video.frame_rate();
                    if let None = fps {
                        fps = Some(self.rate());
                    }
                }
            }
            ffmpeg_next::media::Type::Audio => {
                if let Ok(audio) = codec.decoder().audio() {
                    spec = Some(AudioSpec {
                        freq: Some(audio.rate() as i32),
                        channels: Some(audio.channel_layout().channels()),
                        format: Some(audio.format().convert()),
                    });
                }
            }
            _ => {}
        }
        StreamInfo {
            time_base,
            kind,
            fps,
            audio_spec: spec,
        }
    }
}

impl ConvFormat<StreamInfo> for Parameters {
    fn convert(&self) -> StreamInfo {
        let codec = ffmpeg_next::codec::context::Context::from_parameters(self.clone()).unwrap();
        let time_base = codec.time_base();
        let kind = codec.medium();
        let mut fps = None;
        let mut spec = None;
        match codec.medium() {
            ffmpeg_next::media::Type::Video => {
                if let Ok(video) = codec.decoder().video() {
                    fps = video.frame_rate();
                }
            }
            ffmpeg_next::media::Type::Audio => {
                if let Ok(audio) = codec.decoder().audio() {
                    spec = Some(AudioSpec {
                        freq: Some(audio.rate() as i32),
                        channels: Some(audio.channels() as i32),
                        format: Some(audio.format().convert()),
                    });
                }
            }
            _ => {}
        }
        StreamInfo {
            time_base,
            kind,
            fps,
            audio_spec: spec,
        }
    }
}
