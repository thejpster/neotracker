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
    sample_position: neotracker::Fractional,
    first_pass: bool,
    effect: Option<neotracker::Effect>,
}

struct Player<'a> {
    modfile: neotracker::ProTrackerModule<'a>,
    /// How many samples left in this tick
    samples_left: u32,
    /// How many ticks left in this line
    ticks_left: u32,
    ticks_per_line: u32,
    samples_per_tick: u32,
    clock_ticks_per_device_sample: neotracker::Fractional,
    position: u8,
    line: u8,
    finished: bool,
    channels: [Channel; 4],
}

/// This code is based on https://www.codeslow.com/2019/02/in-this-post-we-will-finally-have-some.html?m=1
impl<'a> Player<'a> {
    /// Make a new player, at the given sample rate.
    fn new(data: Vec<u8>, sample_rate: u32) -> Result<Player<'a>, neotracker::Error> {
        // We need a 'static reference to this data, and we're not going to free it.
        // So just leak it.
        let data_ref: &'static [u8] = data.leak();
        let modfile = neotracker::ProTrackerModule::new(data_ref)?;
        Ok(Player {
            modfile,
            samples_left: 0,
            ticks_left: 0,
            ticks_per_line: 6,
            samples_per_tick: sample_rate / 50,
            position: 0,
            line: 0,
            finished: false,
            clock_ticks_per_device_sample: neotracker::Fractional::new_from_sample_rate(
                sample_rate,
            ),
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
        if self.ticks_left == 0 && self.samples_left == 0 {
            // yes it is time for a new line
            let line = loop {
                // Work out which pattern we're playing
                let Some(pattern_idx) = self.modfile.song_position(self.position) else {
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
                if note.is_empty() {
                    print!("-- --- ----|");
                } else {
                    // 0 means carry on previous note
                    let sample = self.modfile.sample_info(note.sample_no());
                    if let Some(sample) = sample {
                        ch.note_period = note.period();
                        if note.period() != 0 {
                            ch.volume = sample.volume();
                            ch.sample_num = note.sample_no();
                            ch.sample_position = neotracker::Fractional::default();
                            ch.first_pass = true;
                        }
                    }
                    print!(
                        "{:02} {:3} {:04x}|",
                        note.sample_no(),
                        note.musical_note().unwrap_or("--"),
                        note.effect_u16()
                    );
                    ch.effect = None;
                    match note.effect() {
                        e @ Some(
                            neotracker::Effect::Arpeggio(_)
                            | neotracker::Effect::SlideUp(_)
                            | neotracker::Effect::SlideDown(_)
                            | neotracker::Effect::VolumeSlide(_),
                        ) => {
                            // we'll need this for later
                            ch.effect = e;
                        }
                        Some(neotracker::Effect::SetVolume(value)) => {
                            ch.volume = value;
                        }
                        Some(neotracker::Effect::SetSpeed(value)) => {
                            if value <= 31 {
                                self.ticks_per_line = u32::from(value);
                            } else {
                                // They are trying to set speed in beats per minute
                            }
                        }
                        Some(neotracker::Effect::SampleOffset(n)) => {
                            let offset = u32::from(n) * 256;
                            ch.sample_position = neotracker::Fractional::new(offset);
                        }
                        Some(e) => {
                            eprintln!("Unhandled effect {:02x?}", e);
                        }
                        None => {
                            // Do nothing
                        }
                    }
                }
            }
            println!();

            self.line += 1;
            self.samples_left = self.samples_per_tick - 1;
            self.ticks_left = self.ticks_per_line - 1;
        } else if self.samples_left == 0 {
            // end of a tick
            self.samples_left = self.samples_per_tick - 1;
            self.ticks_left -= 1;
            let lower_third = self.ticks_per_line / 3;
            let upper_third = lower_third * 2;
            for ch in self.channels.iter_mut() {
                match ch.effect {
                    Some(neotracker::Effect::Arpeggio(n)) => {
                        if self.ticks_left == upper_third {
                            let half_steps = n >> 4;
                            if let Some(new_period) =
                                neotracker::shift_period(ch.note_period, half_steps)
                            {
                                ch.note_period = new_period;
                            }
                        } else if self.ticks_left == lower_third {
                            let first_half_steps = n >> 4;
                            let second_half_steps = n & 0x0F;
                            if let Some(new_period) = neotracker::shift_period(
                                ch.note_period,
                                second_half_steps - first_half_steps,
                            ) {
                                ch.note_period = new_period;
                            }
                        }
                    }
                    Some(neotracker::Effect::SlideUp(n)) => {
                        ch.note_period -= u16::from(n);
                    }
                    Some(neotracker::Effect::SlideDown(n)) => {
                        ch.note_period += u16::from(n);
                    }
                    Some(neotracker::Effect::VolumeSlide(n)) => {
                        let xxxx = n >> 4;
                        let yyyy = n & 0x0F;
                        if xxxx != 0 {
                            ch.volume = (ch.volume + xxxx).min(63);
                        } else if yyyy != 0 {
                            ch.volume = ch.volume.saturating_sub(yyyy);
                        }
                    }
                    _ => {
                        // do nothing
                    }
                }
            }
        } else {
            // just another sample
            self.samples_left -= 1;
        }

        // Pump existing channels
        let mut left_sample = 0;
        let mut right_sample = 0;
        for (ch_idx, ch) in self.channels.iter_mut().enumerate() {
            if ch.note_period == 0 {
                continue;
            }
            let current_sample = self.modfile.sample(ch.sample_num).expect("bad sample");
            let sample_data = current_sample.raw_sample_bytes();
            if sample_data.len() == 0 {
                continue;
            }
            let integer_pos = ch.sample_position.as_index();
            let sample_byte = sample_data[integer_pos];
            let mut channel_value = sample_byte as i8 as i32;
            // max channel vol (64), sample range [ -128,127] scaled to [-32768, 32767]
            channel_value *= 256;
            channel_value *= i32::from(ch.volume);
            channel_value /= 64;
            ch.sample_position += self
                .clock_ticks_per_device_sample
                .apply_period(ch.note_period);

            let new_integer_pos = ch.sample_position.as_index();
            let limit = if ch.first_pass {
                current_sample.sample_length_bytes()
            } else {
                current_sample.repeat_length_bytes()
            };
            if new_integer_pos >= limit {
                ch.sample_position =
                    neotracker::Fractional::new(current_sample.repeat_point_bytes() as u32);
                ch.first_pass = false;
            }

            if ch_idx == 0 || ch_idx == 3 {
                left_sample += channel_value;
            } else {
                right_sample += channel_value;
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
    // let the buffer empty (it's probably buffered less than a second's worth)
    std::thread::sleep(std::time::Duration::from_secs(1));

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
