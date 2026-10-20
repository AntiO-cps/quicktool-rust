use quicktool_rust::pixels::{Filters, process};
use std::{hint::black_box, time::Instant};
fn main() {
    let mut seed = 0x51475549_u32;
    let input: Vec<u8> = (0..1024 * 1024 * 4)
        .map(|_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 24) as u8
        })
        .collect();
    let filters = Filters {
        brightness: 25.,
        contrast: 18.,
        saturation: 30.,
        exposure: 0.35,
        hue: 12.,
        colorize: false,
        colorize_saturation: 25.,
    };
    for _ in 0..3 {
        let mut data = input.clone();
        process(black_box(&mut data), filters).unwrap();
    }
    let mut samples = Vec::new();
    let mut checksum = 0u64;
    for _ in 0..7 {
        let start = Instant::now();
        let mut data = input.clone();
        process(black_box(&mut data), filters).unwrap();
        samples.push(start.elapsed().as_secs_f64() * 1000.);
        checksum = data.iter().map(|b| *b as u64).sum();
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{}",
        serde_json::json!({"pixels":1024*1024,"samplesMs":samples,"medianMs":samples[3],"checksum":checksum,"profile":"release"})
    );
}
