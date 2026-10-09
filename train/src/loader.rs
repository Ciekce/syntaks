// based on the viri loader in bullet

use crate::inputs::LoadedTakBoard;
use bullet_trainer::reader::DataReader;
use rand::seq::SliceRandom;
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::{self, SyncSender};
use syntaks::board::Position;
use syntaks::core::Score;
use syntaks::format::{Game, GameResult, PackedTakBoard, parse_game, read_game_bytes};
use syntaks::takmove::Move;

pub type TakFilter = fn(&Position, Move, Score, GameResult) -> bool;

#[derive(Clone)]
pub struct TakReader {
    file_paths: Vec<String>,
    buffer_size: usize,
    threads: usize,
    filter: TakFilter,
}

impl TakReader {
    pub fn new(file_paths: Vec<String>, buffer_size_mb: usize, threads: usize, filter: TakFilter) -> Self {
        Self {
            file_paths,
            buffer_size: buffer_size_mb * 1024 * 1024 / size_of::<LoadedTakBoard>() / 2,
            threads,
            filter,
        }
    }
}

impl DataReader<LoadedTakBoard> for TakReader {
    fn read_chunks<F: FnMut(&[LoadedTakBoard]) -> bool>(&self, _: usize, mut f: F) {
        let mut shuffle_buffer = Vec::new();
        shuffle_buffer.reserve_exact(self.buffer_size);

        let file_paths = self.file_paths.clone();
        let buffer_size = self.buffer_size;
        let threads = self.threads;
        let filter = self.filter.clone();

        let (sender, receiver) = mpsc::sync_channel::<Vec<Vec<u8>>>(4);
        let (msg_sender, msg_receiver) = mpsc::sync_channel::<bool>(1);

        std::thread::spawn(move || {
            let mut games = Vec::new();

            'dataloading: loop {
                for file_path in &file_paths {
                    let mut reader = BufReader::new(File::open(file_path.as_str()).unwrap());

                    loop {
                        let mut buf = Vec::new();
                        if !read_game_bytes(&mut reader, &mut buf).unwrap_or(false) {
                            break;
                        }

                        games.push(buf);

                        if games.len().is_multiple_of(8192 * threads) {
                            if msg_receiver.try_recv().unwrap_or(false) || sender.send(games).is_err() {
                                break 'dataloading;
                            }

                            games = Vec::new();
                        }
                    }
                }
            }
        });

        let (game_sender, game_receiver) = mpsc::sync_channel::<Vec<LoadedTakBoard>>(4 * self.threads);
        let (game_msg_sender, game_msg_receiver) = mpsc::sync_channel::<bool>(1);

        std::thread::spawn(move || {
            'dataloading: while let Ok(games) = receiver.recv() {
                if game_msg_receiver.try_recv().unwrap_or(false) {
                    msg_sender.send(true).unwrap();
                    break 'dataloading;
                }

                convert_buffer(threads, &game_sender, &games, &filter);
            }
        });

        let (buffer_sender, buffer_receiver) = mpsc::sync_channel::<Vec<LoadedTakBoard>>(0);
        let (buffer_msg_sender, buffer_msg_receiver) = mpsc::sync_channel::<bool>(1);

        std::thread::spawn(move || {
            'dataloading: while let Ok(game) = game_receiver.recv() {
                if buffer_msg_receiver.try_recv().unwrap_or(false) {
                    game_msg_sender.send(true).unwrap();
                    break 'dataloading;
                }

                if shuffle_buffer.len() + game.len() < shuffle_buffer.capacity() {
                    shuffle_buffer.extend_from_slice(&game);
                } else {
                    let diff = shuffle_buffer.capacity() - shuffle_buffer.len();
                    if diff > 0 {
                        shuffle_buffer.extend_from_slice(&game[..diff]);
                    }

                    shuffle_buffer.shuffle(&mut rand::rng());

                    if buffer_msg_receiver.try_recv().unwrap_or(false) || buffer_sender.send(shuffle_buffer).is_err() {
                        game_msg_sender.send(true).unwrap();
                        break 'dataloading;
                    }

                    shuffle_buffer = Vec::new();
                    shuffle_buffer.reserve_exact(buffer_size);
                    shuffle_buffer.extend_from_slice(&game[diff..]);
                }
            }
        });

        'dataloading: while let Ok(shuffle_buffer) = buffer_receiver.recv() {
            if f(&shuffle_buffer) {
                buffer_msg_sender.send(true).unwrap();
                break 'dataloading;
            }
        }

        drop(buffer_receiver);
    }
}

fn convert_buffer(threads: usize, sender: &SyncSender<Vec<LoadedTakBoard>>, games: &[Vec<u8>], filter: &TakFilter) {
    let chunk_size = games.len().div_ceil(threads);

    std::thread::scope(|s| {
        for chunk in games.chunks(chunk_size) {
            let this_sender = sender.clone();
            s.spawn(move || {
                let mut buffer = Vec::new();

                for game_bytes in chunk {
                    let game = parse_game(game_bytes).unwrap();
                    splat(&game, &mut buffer, filter);
                }

                this_sender.send(buffer)
            });
        }
    });
}

fn splat(game: &Game, out: &mut Vec<LoadedTakBoard>, filter: &TakFilter) {
    let mut pos = game.root;

    for &(mv, eval) in &game.moves {
        if filter(&pos, mv, eval as Score, game.result) {
            out.push(LoadedTakBoard(PackedTakBoard::encode_position_from_game(
                &pos,
                eval as Score,
                game.result,
            )));
        }
        pos = pos.apply_move(mv);
    }
}
