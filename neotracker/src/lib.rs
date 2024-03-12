//! Basic Pro Tracker module parser
//!
//! Based upon https://www.eblong.com/zarf/blorb/mod-spec.txt.

#![no_std]
#![deny(missing_docs)]

/// The ways in which parsing can fail
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file was not large enough to contain a MOD header
    FileTooSmall,
    /// The file did not contain a recognised magic value
    WrongMagicValue,
}

/// Represents a Pro Tracker Module.
///
/// Stores no data - just holds a &[u8] containing the raw file contents.
pub struct ProTrackerModule<'a> {
    data: &'a [u8],
}

impl<'a> ProTrackerModule<'a> {
    const MINIMUM_LENGTH: usize = 1084 + 1024;
    const SONG_LENGTH_OFFSET: usize = 950;
    const SONG_POSITIONS_RANGE: core::ops::Range<usize> = 952..1080;
    const MK_RANGE: core::ops::Range<usize> = 1080..1084;
    const MK_MAGIC: [u8; 4] = [b'M', b'.', b'K', b'.'];

    /// Create a wrapper around a MOD file already in memory.
    ///
    /// Does some basic checks to ensure it looks like a MOD file.
    pub fn new(data: &'a [u8]) -> Result<ProTrackerModule<'a>, Error> {
        if data.len() < Self::MINIMUM_LENGTH {
            return Err(Error::FileTooSmall);
        }
        if &data[Self::MK_RANGE] != &Self::MK_MAGIC {
            return Err(Error::WrongMagicValue);
        }
        Ok(ProTrackerModule { data })
    }

    /// Iterate through all the samples
    pub fn samples(&self) -> SampleIter {
        SampleIter {
            parent: self,
            sample_no: 0,
            file_offset: self.sample_offset(),
        }
    }

    /// Get info on a specific sample.
    ///
    /// The value is 1-indexed.
    ///
    /// Requires a walk through all the samples so we can
    /// get the start of the sample data.
    pub fn sample(&self, sample_no: u8) -> Option<Sample> {
        if sample_no == 0 {
            None
        } else {
            self.samples().nth(usize::from(sample_no - 1))
        }
    }

    /// Get metadata for a specific sample
    ///
    /// Can do a direct access, but it won't return correct sample data.
    pub fn sample_info(&self, sample_no: u8) -> Option<Sample> {
        if sample_no >= 1 && sample_no <= 30 {
            Some(Sample {
                parent: self,
                sample_no: sample_no - 1,
                // this value is wrong, but we did warn them it would be
                file_offset: self.sample_offset(),
            })
        } else {
            None
        }
    }

    /// Number patterns that make up the song.
    pub fn song_length(&self) -> u8 {
        self.data[Self::SONG_LENGTH_OFFSET]
    }

    /// Which pattern should be played at this song position
    ///
    /// The `idx` argument should be in the range 0..=127.
    pub fn song_position(&self, idx: u8) -> Option<u8> {
        let positions = self.song_positions();
        positions.get(usize::from(idx)).cloned()
    }

    /// Get the list of all the patterns in the song.
    pub fn song_positions(&self) -> &[u8] {
        let length = usize::from(self.song_length());
        &self.data[Self::SONG_POSITIONS_RANGE][0..length]
    }

    /// Return the number of patterns in the file
    pub fn num_patterns(&self) -> u8 {
        *self.data[Self::SONG_POSITIONS_RANGE].iter().max().unwrap() + 1
    }

    /// Get info on a specific pattern
    pub fn pattern(&self, pattern_no: u8) -> Option<Pattern> {
        if pattern_no < self.num_patterns() {
            Some(Pattern {
                pattern_no,
                parent: self,
            })
        } else {
            None
        }
    }

    /// Where in the file do the samples start?
    fn sample_offset(&self) -> usize {
        Pattern::PATTERN_INFO_OFFSET + (usize::from(self.num_patterns()) * Pattern::PATTERN_LEN)
    }
}

/// Represents a pattern
///
/// A pattern is 1024 bytes, comprised of 64 notes, with 4 channels per note and 4 bytes per channel.
pub struct Pattern<'a> {
    pattern_no: u8,
    parent: &'a ProTrackerModule<'a>,
}

impl<'a> Pattern<'a> {
    const PATTERN_INFO_OFFSET: usize = 1084;
    const PATTERN_LEN: usize = 1024;

    fn metadata_bytes(&self) -> &[u8] {
        let start = Self::PATTERN_INFO_OFFSET + (usize::from(self.pattern_no) * Self::PATTERN_LEN);
        let end = start + Self::PATTERN_LEN;
        &self.parent.data[start..end]
    }

    /// Grab one specific line from a pattern
    pub fn line(&self, index: u8) -> Option<Line<4>> {
        let mut iter = LineIter {
            note: index,
            parent: self,
        };
        iter.next()
    }

    /// Iterate through all the lines in a pattern
    pub fn lines(&self) -> LineIter {
        LineIter {
            note: 0,
            parent: self,
        }
    }
}

/// Lets you iterate through the notes in a pattern
pub struct LineIter<'a> {
    note: u8,
    parent: &'a Pattern<'a>,
}

impl<'a> Iterator for LineIter<'a> {
    type Item = Line<4>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.note >= 64 {
            return None;
        }
        let data = self.parent.metadata_bytes();
        let offset = usize::from(self.note) * 16;
        self.note += 1;
        Some(Line {
            channel: [
                Note {
                    data: [
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ],
                },
                Note {
                    data: [
                        data[offset + 4],
                        data[offset + 5],
                        data[offset + 6],
                        data[offset + 7],
                    ],
                },
                Note {
                    data: [
                        data[offset + 8],
                        data[offset + 9],
                        data[offset + 10],
                        data[offset + 11],
                    ],
                },
                Note {
                    data: [
                        data[offset + 12],
                        data[offset + 13],
                        data[offset + 14],
                        data[offset + 15],
                    ],
                },
            ],
        })
    }
}

/// A set of notes, one per channel, for a line in a pattern.
pub struct Line<const NUM_CHANNELS: usize> {
    /// An array of channels
    pub channel: [Note; NUM_CHANNELS],
}

/// Conversion from period to musical note
pub static PERIOD_NOTE_MAP: &[(u16, &str)] = &[
    (856, "C1"),
    (808, "C1♯"),
    (762, "D1"),
    (720, "D1♯"),
    (678, "E1"),
    (640, "F1"),
    (604, "F1♯"),
    (570, "G1"),
    (538, "G1♯"),
    (508, "A1"),
    (480, "A1♯"),
    (453, "B1"),
    (428, "C2"),
    (404, "C2♯"),
    (381, "D2"),
    (360, "D2♯"),
    (339, "E2"),
    (320, "F2"),
    (302, "F2♯"),
    (285, "G2"),
    (269, "G2♯"),
    (254, "A2"),
    (240, "A2♯"),
    (226, "B2"),
    (214, "C3"),
    (202, "C3♯"),
    (190, "D3"),
    (180, "D3♯"),
    (170, "E3"),
    (160, "F3"),
    (151, "F3♯"),
    (143, "G3"),
    (135, "G3♯"),
    (127, "A3"),
    (120, "A3♯"),
    (113, "B3"),
];

/// Move a period up by a number of half-steps
///
/// Used for Arpeggios
pub fn shift_period(period: u16, half_steps: u8) -> Option<u16> {
    if let Ok(idx) = PERIOD_NOTE_MAP.binary_search_by(|(n, _)| n.cmp(&period)) {
        PERIOD_NOTE_MAP.get(idx + half_steps as usize).map(|n| n.0)
    } else {
        None
    }
}

/// A note that can be played on a given channel.
pub struct Note {
    data: [u8; 4],
}

impl Note {
    /// Get which sample should be played
    pub fn sample_no(&self) -> u8 {
        self.data[0] & 0xF0 | (self.data[2] & 0xF0) >> 4
    }

    /// Get the sample period (i.e. pitch)
    ///
    /// In the range 0..4096
    pub fn period(&self) -> u16 {
        u16::from(self.data[0] & 0x0F) << 8 | u16::from(self.data[1])
    }

    /// The musical note, if any, this note matches
    pub fn musical_note(&self) -> Option<&'static str> {
        let period = self.period();
        let position = PERIOD_NOTE_MAP
            .binary_search_by(|probe| period.cmp(&probe.0))
            .ok();
        if let Some(position) = position {
            PERIOD_NOTE_MAP.get(position).map(|info| info.1)
        } else {
            None
        }
    }

    /// Get the effect command
    pub fn effect(&self) -> Option<Effect> {
        Effect::try_from(self.effect_u16())
    }

    /// Get the effect command
    ///
    /// In the format 0x0NMM where N is the command and MM is the argument
    pub fn effect_u16(&self) -> u16 {
        u16::from(self.data[2] & 0x0F) << 8 | u16::from(self.data[3])
    }

    /// Does this note do nothing?
    pub fn is_empty(&self) -> bool {
        self.effect_u16() == 0 && self.period() == 0 && self.sample_no() == 0
    }
}

/// Represents an effect
#[repr(u8)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Effect {
    /// Arpeggio
    Arpeggio(u8) = 0,
    /// Slide up
    SlideUp(u8) = 1,
    /// Slide down
    SlideDown(u8) = 2,
    /// Slide to note
    SlideToNote(u8) = 3,
    /// Vibrato
    Vibrato(u8) = 4,
    /// Slide to note and volume slide
    SlideNoteVolume(u8) = 5,
    /// Vibrato and volume slide
    VibratoSlide(u8) = 6,
    /// Tremolo
    Tremelo(u8) = 7,
    /// Set sample offset
    SampleOffset(u8) = 9,
    /// Volume slide
    VolumeSlide(u8) = 10,
    /// Position jump
    PositionJump(u8) = 11,
    /// Set volume
    SetVolume(u8) = 12,
    /// Pattern break
    PatternBreak(u8) = 13,
    /// Set speed
    SetSpeed(u8) = 15,
}

impl Effect {
    /// Try and parse a 16-bit effect value
    pub const fn try_from(value: u16) -> Option<Effect> {
        if value == 0 {
            return None;
        }
        let arg = (value & 0xFF) as u8;
        match value >> 8 {
            0 => Some(Effect::Arpeggio(arg)),
            1 => Some(Effect::SlideUp(arg)),
            2 => Some(Effect::SlideDown(arg)),
            3 => Some(Effect::SlideToNote(arg)),
            4 => Some(Effect::Vibrato(arg)),
            5 => Some(Effect::SlideNoteVolume(arg)),
            6 => Some(Effect::VibratoSlide(arg)),
            7 => Some(Effect::Tremelo(arg)),
            9 => Some(Effect::SampleOffset(arg)),
            10 => Some(Effect::VolumeSlide(arg)),
            11 => Some(Effect::PositionJump(arg)),
            12 => Some(Effect::SetVolume(arg)),
            13 => Some(Effect::PatternBreak(arg)),
            15 => Some(Effect::SetSpeed(arg)),
            _ => None,
        }
    }
}

/// Represents a sample
pub struct Sample<'a> {
    /// A zero-based indexed into the sample array
    sample_no: u8,
    /// Where in the MOD file the sample starts
    file_offset: usize,
    /// The MOD file itself
    parent: &'a ProTrackerModule<'a>,
}

impl<'a> Sample<'a> {
    const SAMPLE_INFO_OFFSET: usize = 20;
    const SAMPLE_INFO_LEN: usize = 30;
    const SAMPLE_MAX_NAME_LEN: usize = 22;

    fn metadata_bytes(&self) -> &[u8] {
        let start =
            Self::SAMPLE_INFO_OFFSET + (usize::from(self.sample_no) * Self::SAMPLE_INFO_LEN);
        let end = start + Self::SAMPLE_INFO_LEN;
        &self.parent.data[start..end]
    }

    /// The name of the sample, as a byte slice.
    ///
    /// Is probably not UTF-8 encoded.
    pub fn name(&self) -> &[u8] {
        let mut name: &[u8] = &self.metadata_bytes()[0..Self::SAMPLE_MAX_NAME_LEN];
        while let Some(trimmed_name) = name.strip_suffix(b"\0") {
            name = trimmed_name;
        }
        name
    }

    /// Length of the sample, in 16-bit units
    pub fn sample_length(&self) -> u16 {
        let len: &[u8] = &self.metadata_bytes()[22..24];
        u16::from_be_bytes(len.try_into().unwrap())
    }

    /// Length of the sample in bytes
    pub fn sample_length_bytes(&self) -> usize {
        usize::from(self.sample_length()) * 2
    }

    /// The finetune value for the sample
    pub fn finetune(&self) -> u8 {
        self.metadata_bytes()[24]
    }

    /// The default volume of the sample
    pub fn volume(&self) -> u8 {
        self.metadata_bytes()[25]
    }

    /// Where the sample should loop back to when repeating, in 16-bit units.
    pub fn repeat_point(&self) -> u16 {
        let len: &[u8] = &self.metadata_bytes()[26..28];
        u16::from_be_bytes(len.try_into().unwrap())
    }

    /// Where the sample should loop back to when repeating, as a byte offset.
    pub fn repeat_point_bytes(&self) -> usize {
        usize::from(self.repeat_point()) * 2
    }

    /// The length of the repeating portion, in 16-bit units
    pub fn repeat_length(&self) -> u16 {
        let len: &[u8] = &self.metadata_bytes()[28..30];
        u16::from_be_bytes(len.try_into().unwrap())
    }

    /// The length of the repeating portion, in bytes
    pub fn repeat_length_bytes(&self) -> usize {
        usize::from(self.repeat_length()) * 2
    }

    /// The sample as 8-bit data
    pub fn raw_sample_bytes(&self) -> &[u8] {
        let range = self.file_offset..self.file_offset + self.sample_length_bytes();
        &self.parent.data[range]
    }

    /// Create an iterator that will hand out samples, handling looping/repeating as required.
    pub fn sample_bytes_iter(&'a self) -> SampleBytesIter<'a> {
        SampleBytesIter {
            data: self.raw_sample_bytes(),
            repeat_length: self.repeat_length_bytes(),
            repeat_point: self.repeat_point_bytes(),
            first_pass: true,
            position: 0,
        }
    }
}

/// Generates the infinite 1 byte PCM samples contained within a sample.
pub struct SampleBytesIter<'a> {
    data: &'a [u8],
    repeat_point: usize,
    repeat_length: usize,
    first_pass: bool,
    position: usize,
}

impl<'a> Iterator for SampleBytesIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        let sample = self.data.get(self.position).cloned();
        self.position += 1;
        if self.first_pass {
            if self.position >= self.data.len() {
                self.position = self.repeat_point;
                self.first_pass = false;
            }
        } else {
            if self.position >= self.repeat_point + self.repeat_length {
                self.position = self.repeat_point;
            }
        }
        sample
    }
}

/// Generated by [`ProTrackerModule::samples()`].
pub struct SampleIter<'a> {
    parent: &'a ProTrackerModule<'a>,
    sample_no: u8,
    file_offset: usize,
}

impl<'a> core::iter::Iterator for SampleIter<'a> {
    type Item = Sample<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_no <= 30 {
            let sample = Sample {
                parent: self.parent,
                sample_no: self.sample_no,
                file_offset: self.file_offset,
            };
            self.sample_no += 1;
            self.file_offset += sample.sample_length_bytes();
            Some(sample)
        } else {
            None
        }
    }
}

/// Represents a fixed-point 24.8 bit value
///
/// Useful for calculating sample indicies.
#[derive(Debug, Copy, Clone, Default)]
pub struct Fractional {
    inner: u32,
}

impl Fractional {
    const AMIGA_CLOCK: u32 = 3_546_895;

    /// Create a new fractional value
    pub const fn new(value: u32) -> Fractional {
        Fractional { inner: value << 8 }
    }

    /// Create a new fractional value from the Amiga clock rate
    pub const fn new_from_sample_rate(sample_rate: u32) -> Fractional {
        Fractional {
            inner: (Self::AMIGA_CLOCK * 256) / sample_rate,
        }
    }

    /// Convert to a sample index
    pub const fn as_index(self) -> usize {
        (self.inner >> 8) as usize
    }

    /// Divide this fractional value by the given period
    pub fn apply_period(self, period: u16) -> Fractional {
        Fractional {
            inner: self.inner / u32::from(period),
        }
    }
}

impl core::ops::Add for Fractional {
    type Output = Fractional;

    fn add(self, rhs: Self) -> Self::Output {
        Fractional {
            inner: self.inner + rhs.inner,
        }
    }
}

impl core::ops::AddAssign for Fractional {
    fn add_assign(&mut self, rhs: Self) {
        self.inner = self.inner + rhs.inner;
    }
}

// End of file
