use std::{
    io::{self, BufRead},
    process::exit,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use ffmpeg_next::Rational;

use crate::mplayer::MPlayer;

mod audio;
mod constants;
mod convert;
mod core;
mod mplayer;
mod utils;

fn main() {
    let (tx, rx) = mpsc::channel::<Command>();

    // Commander thread: handles user input and sends commands
    let commander_thread = thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                match line.as_str() {
                    "exit" => {
                        let _ = tx.send(Command::Shutdown);
                        println!("shutting down mplayer");
                    }
                    _ if line.contains("open") => {
                        if let Some(dir) = line.split("open").nth(1) {
                            let _ = tx.send(Command::Play(String::from(
                                dir.replace("\"", "").replace("'", "").trim(),
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    let player = MPlayer::setup();

    match player {
        Ok(mut player) => {
            player.go(rx, Rational(1, 100));
        }
        Err(err) => {
            println!("{:?}", err);
        }
    }

    // Wait for commander thread to finish
    let _ = commander_thread.join();
}

#[derive(Clone)]
pub enum Command {
    Shutdown,
    Play(String),
    Pause,
    Goto(u32),
}
