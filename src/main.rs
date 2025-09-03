use std::{
    io::{self, BufRead},
    process::exit,
    sync::mpsc,
    thread,
    time::Instant,
};

use crate::mplayer::MPlayer;

mod audio;
mod constants;
mod decode;
mod mplayer;
mod utils;
mod core;

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
                        break;
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
    let mut tick_count: u64 = 0;
    // Player runs in main thread
    let mut timer = Instant::now();
    let player = MPlayer::setup();
    match player {
        Ok(mut player) => loop {
            tick_count += 1;
            if timer.elapsed().as_secs_f32() > 1.0 {
                println!(
                    "[TPS] {}",
                    tick_count as f32 / timer.elapsed().as_secs_f32()
                );
                tick_count = 0;
                timer = Instant::now();
            }
            if player.should_exit {
                exit(0);
            }
            let mut cli_command = None;
            if let Ok(command) = rx.try_recv() {
                cli_command = Some(command.clone());
                if let Command::Shutdown = command {
                    break;
                }
            }

            player.tick(cli_command);
        },
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
