use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crate::ui::palette::*;

/// Purple-core pulse with a brief rouge tang at the peaks.
const PULSES: &[(u8, u8, u8)] = &[
    (30, 8, 45),
    (50, 15, 70),
    (75, 25, 105),
    (100, 35, 140),
    (130, 50, 180), // violet
    (155, 70, 200),
    (175, 85, 210), // bright amethyst
    (195, 70, 185), // rouge bleeds in
    (200, 55, 145), // rouge tang peak
    (190, 60, 165), // warming back
    (170, 70, 190), // back to purple
    (140, 50, 170),
    (100, 30, 130), // nightshade trough
    (75, 20, 100),
    (50, 12, 70),  // plum
    (75, 25, 105), // rising again
    (110, 40, 150),
    (140, 55, 185),
    (165, 75, 205), // amethyst
    (180, 85, 215), // peak
    (195, 68, 180), // rouge whisper
    (175, 72, 195), // settling
    (145, 55, 175),
    (115, 40, 150),
    (90, 30, 130), // settle on NIGHTSHADE
];

pub fn render() {
    let mut err = io::stderr();

    // Hide cursor during animation
    write!(err, "\x1b[?25l").ok();

    for &(r, g, b) in PULSES {
        write!(err, "\r  \x1b[38;2;{r};{g};{b}m\x1b[1m◈ nyx\x1b[0m").ok();
        err.flush().ok();
        thread::sleep(Duration::from_millis(40));
    }

    // Show cursor, finish line
    write!(err, "\x1b[?25h").ok();
    eprintln!();

    let p = ansi_fg(PLUM);
    eprintln!("  {p}{ANSI_DIM}─────────────────────────────{ANSI_RESET}");
}
