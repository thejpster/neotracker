//! Plays a MOD file using cpal.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

static STOP_PLAYING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Default)]
struct Channel {
    sample_num: u8,
    volume: u8,
    note_period: u16,
    /// As a u16:u16 fractional value
    sample_position: u32,
    first_pass: bool,
}

struct Player<'a> {
    modfile: neotracker::ProTrackerModule<'a>,
    sample_clock: u32,
    sample_rate: u32,
    samples_per_line: u32,
    /// As a u16:u16 fractional value
    clock_ticks_per_device_sample: u32,
    position: u8,
    line: u8,
    finished: bool,
    channels: [Channel; 4],
}

/// This code is based on https://www.codeslow.com/2019/02/in-this-post-we-will-finally-have-some.html?m=1
impl<'a> Player<'a> {
    const AMIGA_CLOCK: u32 = 3_546_895;

    /// Make a new player, at the given sample rate.
    fn new(data: Vec<u8>, sample_rate: u32) -> Result<Player<'a>, neotracker::Error> {
        // We need a 'static reference to this data, and we're not going to free it.
        // So just leak it.
        let data_ref: &'static [u8] = data.leak();
        let modfile = neotracker::ProTrackerModule::new(data_ref)?;
        Ok(Player {
            modfile,
            sample_clock: 0,
            sample_rate,
            samples_per_line: 6 * (sample_rate / 50),
            position: 0,
            line: 0,
            finished: false,
            // Trying hard not to overflow a u32 here
            clock_ticks_per_device_sample: ((1024 * Self::AMIGA_CLOCK) / sample_rate) * 64,
            channels: [
                Channel::default(),
                Channel::default(),
                Channel::default(),
                Channel::default(),
            ],
        })
    }

    /// Return a stereo sample pair
    fn next_sample(&mut self) -> (i16, i16) {
        // Time for a new line?
        if self.sample_clock == 0 {
            // yes it is time for a new line
            let line = loop {
                // Work out which pattern we're playing
                let Ok(pattern_idx) = self.modfile.song_position(self.position) else {
                    self.finished = true;
                    return (0, 0);
                };
                // Grab the pattern
                let pattern = self.modfile.pattern(pattern_idx).expect("Get pattern");
                // Get the line from the pattern
                let Some(line) = pattern.line(self.line) else {
                    // Go to start of next pattern
                    self.line = 0;
                    self.position += 1;
                    continue;
                };
                break line;
            };

            // Load four channels with new line data
            print!("{:03} {:06}: ", self.position, self.line);
            for (channel_num, ch) in self.channels.iter_mut().enumerate() {
                let note = &line.channel[channel_num];
                // 0 means carry on previous note
                if note.sample_no() != 0 {
                    if note.period() != 0 {
                        ch.sample_num = note.sample_no();
                        ch.volume = 64;
                        ch.sample_position = 0;
                        ch.note_period = note.period();
                        ch.first_pass = true;
                    } else {
                        ch.volume = 0;
                    }
                    print!(
                        "{:02} {:3} {:04x}|",
                        note.sample_no(),
                        note.musical_note().unwrap_or("--"),
                        note.effect()
                    );
                } else {
                    print!("-- --- ----|");
                }
                if note.effect() != 0 {
                    let effect = note.effect();
                    let command = effect >> 8;
                    let value = effect & 0xFF;
                    match command {
                        0x04 => {
                            // Vibrato
                        }
                        0x0C => {
                            // Set volume
                            ch.volume = value as u8;
                        }
                        0x0A => {
                            // Volume slide
                        }
                        0x0F => {
                            // Set speed
                            self.samples_per_line = (value as u32 * self.sample_rate) / 50;
                        }
                        _ => {
                            eprintln!("Unhandled effect {:#04x}", effect);
                        }
                    }
                }
            }
            println!();

            self.line += 1;
        }
        self.sample_clock += 1;
        if self.sample_clock >= self.samples_per_line {
            self.sample_clock = 0;
        }

        // Pump existing channels
        let mut left_sample = 0;
        let mut right_sample = 0;
        for (ch_idx, ch) in self.channels.iter_mut().enumerate() {
            if ch.sample_num != 0 {
                let current_sample = self
                    .modfile
                    .sample(ch.sample_num.saturating_sub(1))
                    .expect("bad sample");
                let sample_data = current_sample.raw_sample_bytes();
                let integer_pos = (ch.sample_position >> 16) as usize;
                if let Some(sample_byte) = sample_data.get(integer_pos) {
                    let mut channel_value = *sample_byte as i8 as i32;
                    // max channel vol (64), sample range [ -128,127] scaled to [-32768, 32767]
                    channel_value *= 256;
                    channel_value *= i32::from(ch.volume);
                    channel_value /= 64;
                    ch.sample_position +=
                        self.clock_ticks_per_device_sample / u32::from(ch.note_period);

                    let new_integer_pos = (ch.sample_position >> 16) as usize;
                    if ch.first_pass {
                        if new_integer_pos >= current_sample.sample_length_bytes() {
                            ch.sample_position = (current_sample.repeat_point_bytes() << 16) as u32;
                            ch.first_pass = false;
                        }
                    } else {
                        if new_integer_pos >= current_sample.repeat_length_bytes() {
                            ch.sample_position = (current_sample.repeat_point_bytes() << 16) as u32;
                        }
                    }

                    if ch_idx == 0 || ch_idx == 3 {
                        left_sample += channel_value;
                    } else {
                        right_sample += channel_value;
                    }
                }
            }
        }
        (
            left_sample.clamp(-32768, 32767) as i16,
            right_sample.clamp(-32768, 32767) as i16,
        )
    }
}

fn main() -> Result<(), anyhow::Error> {
    let data = open_file()?;
    let sample_rate = 44100;

    let mut player =
        Player::new(data, sample_rate).map_err(|e| anyhow::anyhow!("neotracker error: {:?}", e))?;
    println!(
        "Valid MOD file with {} patterns",
        player.modfile.num_patterns()
    );

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No output device found"))?;
    let supported_configs_iter = device.supported_output_configs()?;
    let supported_config = supported_configs_iter
        .filter(|sc| sc.sample_format() == cpal::SampleFormat::I16)
        .filter(|sc| sc.channels() == 2)
        .next()
        .expect("no supported I16 config?!")
        .with_sample_rate(cpal::SampleRate(sample_rate as u32));
    println!("Found config: {:?}", supported_config);
    let config: cpal::StreamConfig = supported_config.into();
    let stream = device.build_output_stream(
        &config,
        move |buffer: &mut [i16], _info| {
            for sample in buffer.chunks_exact_mut(2) {
                let (left, right) = player.next_sample();
                sample[0] = left;
                sample[1] = right;
            }
            if player.finished {
                STOP_PLAYING.store(true, Ordering::Relaxed);
            }
        },
        |err| eprintln!("an error occurred on the output audio stream: {}", err),
        None,
    )?;

    stream.play()?;

    // Play for 1 second. During this delay, the audio engine will call
    // the closures supplied above to generate new samples as required.
    while !STOP_PLAYING.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    Ok(())
}

/// Open and read the first file given on the command line as a `Vec<u8>`.
fn open_file() -> Result<Vec<u8>, anyhow::Error> {
    println!("Player starting...");
    let filename = std::env::args_os()
        .skip(1)
        .take(1)
        .next()
        .ok_or_else(|| anyhow::anyhow!("Need filename as argument"))?;
    let filename: &Path = filename.as_os_str().as_ref();
    println!("Loading {}...", filename.display());
    let data = std::fs::read(filename)?;
    println!("Loaded {} bytes", data.len());
    Ok(data)
}
