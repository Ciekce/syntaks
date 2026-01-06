use crate::perft::{perft, split_perft};

mod bitboard;
mod board;
mod core;
mod movegen;
mod perft;
mod takmove;

fn main() {
    let positions = [
        (
            "x6/x6/x6/x6/x6/x6 1 1",
            [36, 1260, 132720, 13586048, 1253506520].as_slice(),
        ),
        (
            "x,2,2,22S,2,111S/21S,22C,112,x,1112S,11S/x,2,112212,2,2S,2/x,2,121122,x,1112,211/21C,x,1,2S,21S,x/2S,x,212,1S,12S,1S 1 33",
            [56, 17322, 1519011, 328068019].as_slice(),
        ),
        (
            "x2,2,22,2C,1/21221S,1112,x,2211,1,2/x2,111S,x,11S,12S/11S,1S,2S,2,12S,1211C/x,12S,2,122S,x,212S/12,x2,1S,22222S,21121 2 31",
            [108, 13586, 1380694, 153726314].as_slice(),
        ),
        (
            "2,x,2,111S,2,12/2,122S,2122,1S,x,1/x,111,1,11S,x2/21122112C,x,212S,2S,2,1212S/1,112S,21221S,2S,x2/21,222,x,12S,x2 2 30",
            [197, 16949, 2892705, 266189890].as_slice(),
        ),
        (
            "x6/x6/x6/x3,111222111222111222111222111222111222111222111222111222111222C,x2/x6/x6 2 31",
            [194, 13714, 1798660, 160643820, 18275539844].as_slice(),
        ),
        (
            "x6/x4,1S,x/x2,21111S,1C,22122C,x/x6/x6/x6 2 11",
            [95, 11683, 1035124, 111863932].as_slice(),
        ),
    ];

    'outer: for (tps, values) in positions {
        println!("tps: {}", tps);
        let pos = tps.parse().unwrap();
        for (depth, &reference) in values.iter().enumerate() {
            let depth = depth as i32 + 1;
            print!("  depth {}, expected {}, got ", depth, reference);
            let nodes = perft(&pos, depth);
            println!("{}", nodes);
            if nodes != reference {
                println!("Failed");
                split_perft(&pos, depth);
                break 'outer;
            }
        }
    }
}
