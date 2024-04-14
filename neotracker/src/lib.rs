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
        if data[Self::MK_RANGE] != Self::MK_MAGIC {
            return Err(Error::WrongMagicValue);
        }
        Ok(ProTrackerModule { data })
    }

    /// Iterate through all the samples
    pub fn samples(&self) -> SampleIter {
        SampleIter {
            parent: self,
            sample_no: 1,
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
            // `nth` is zero-indexed
            self.samples().nth(usize::from(sample_no - 1))
        }
    }

    /// Get metadata for a specific sample
    ///
    /// Can do a direct access, but it won't return correct sample data.
    pub fn sample_info(&self, sample_no: u8) -> Option<Sample> {
        if (1..=31).contains(&sample_no) {
            // this value is wrong, but we did warn them it would be
            Some(Sample::new(sample_no, self.sample_offset(), self))
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

impl<'a> core::fmt::Debug for ProTrackerModule<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProTrackerModule")
            .field("data", &self.data.len())
            .field("song_length", &self.song_length())
            .field("num_patterns", &self.num_patterns())
            .field("sample_offset", &self.sample_offset())
            .finish()
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
    (856, "C-1"),
    (808, "C1♯"),
    (762, "D-1"),
    (720, "D1♯"),
    (678, "E-1"),
    (640, "F-1"),
    (604, "F1♯"),
    (570, "G-1"),
    (538, "G1♯"),
    (508, "A-1"),
    (480, "A1♯"),
    (453, "B-1"),
    (428, "C-2"),
    (404, "C2♯"),
    (381, "D-2"),
    (360, "D2♯"),
    (339, "E-2"),
    (320, "F-2"),
    (302, "F2♯"),
    (285, "G-2"),
    (269, "G2♯"),
    (254, "A-2"),
    (240, "A2♯"),
    (226, "B-2"),
    (214, "C-3"),
    (202, "C3♯"),
    (190, "D-3"),
    (180, "D3♯"),
    (170, "E-3"),
    (160, "F-3"),
    (151, "F3♯"),
    (143, "G-3"),
    (135, "G3♯"),
    (127, "A-3"),
    (120, "A3♯"),
    (113, "B-3"),
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
    VolumeSlide(i8) = 10,
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
            10 => Some(if arg >= 0x10 {
                Effect::VolumeSlide((arg >> 4) as i8)
            } else {
                Effect::VolumeSlide(-(arg as i8))
            }),
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
    /// A one-based indexed into the sample table
    sample_no: u8,
    /// A volume, from 0 to 63
    volume: u8,
    /// Finetune
    finetune: u8,
    /// Where in the MOD file the sample starts
    file_offset: usize,
    /// The repeat length
    repeat_length: u16,
    /// The repeat point
    repeat_point: u16,
    /// The sample length
    sample_length: u16,
    /// The MOD file itself
    parent: &'a ProTrackerModule<'a>,
}

impl<'a> core::fmt::Debug for Sample<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sample")
            .field("sample_no", &self.sample_no)
            .field("file_offset", &self.file_offset)
            .field("name", &core::str::from_utf8(self.name()).unwrap_or("?"))
            .field("sample_length_bytes", &self.sample_length_bytes())
            .field("finetune", &self.finetune())
            .field("volume", &self.volume())
            .field("repeat_point", &self.repeat_point_bytes())
            .field("repeat_length", &self.repeat_length_bytes())
            .finish()
    }
}

impl<'a> Sample<'a> {
    const SAMPLE_INFO_OFFSET: usize = 20;
    const SAMPLE_INFO_LEN: usize = 30;
    const SAMPLE_MAX_NAME_LEN: usize = 22;

    /// Create a new sample
    ///
    /// The sample_no must be `1..=31`.
    fn new(sample_no: u8, file_offset: usize, parent: &'a ProTrackerModule<'a>) -> Sample<'a> {
        let mut s = Sample {
            sample_no,
            file_offset,
            parent,
            volume: 0,
            finetune: 0,
            repeat_length: 0,
            repeat_point: 0,
            sample_length: 0,
        };
        // Cache the important fields from the metadata
        let metdata_bytes = s.metadata_bytes();
        let sample_length = u16::from_be_bytes([metdata_bytes[22], metdata_bytes[23]]);
        let finetune = metdata_bytes[24];
        let volume = metdata_bytes[25];
        let repeat_point = u16::from_be_bytes([metdata_bytes[26], metdata_bytes[27]]);
        let repeat_length = u16::from_be_bytes([metdata_bytes[28], metdata_bytes[29]]);
        // Now store the cached data
        s.volume = volume;
        s.finetune = finetune;
        s.sample_length = sample_length;
        s.repeat_point = repeat_point;
        s.repeat_length = repeat_length;
        s
    }

    /// Grab the slice of bytes corresponding to this sample's metadata.
    fn metadata_bytes(&self) -> &[u8] {
        let start =
            Self::SAMPLE_INFO_OFFSET + (usize::from(self.sample_no - 1) * Self::SAMPLE_INFO_LEN);
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
        self.sample_length
    }

    /// Length of the sample in bytes
    pub fn sample_length_bytes(&self) -> usize {
        usize::from(self.sample_length * 2)
    }

    /// The finetune value for the sample
    pub fn finetune(&self) -> u8 {
        self.finetune
    }

    /// The default volume of the sample
    pub fn volume(&self) -> u8 {
        self.volume
    }

    /// Does this sample repeat?
    pub fn loops(&self) -> bool {
        self.repeat_length != 1
    }

    /// Where the sample should loop back to when repeating, in 16-bit units.
    pub fn repeat_point(&self) -> u16 {
        self.repeat_point
    }

    /// Where the sample should loop back to when repeating, as a byte offset.
    pub fn repeat_point_bytes(&self) -> usize {
        usize::from(self.repeat_point * 2)
    }

    /// The length of the repeating portion, in 16-bit units
    pub fn repeat_length(&self) -> u16 {
        self.repeat_length
    }

    /// The length of the repeating portion, in bytes
    pub fn repeat_length_bytes(&self) -> usize {
        usize::from(self.repeat_length * 2)
    }

    /// The sample as 8-bit data
    pub fn raw_sample_bytes(&self) -> &[u8] {
        // short-cut if sample is empty
        if self.sample_length == 0 || self.volume == 0 {
            return &[];
        };
        // This is where in the file the sample lives.
        let range = self.file_offset..(self.file_offset + self.sample_length_bytes());
        self.parent.data.get(range).unwrap_or_else(|| {
            // This sample goes off the end of the file. Give them as much as we
            // can instead.
            &self.parent.data[self.file_offset..]
        })
    }

    /// Create an iterator that will hand out samples, handling looping/repeating as required.
    pub fn sample_bytes_iter(&'a self) -> SampleBytesIter<'a> {
        SampleBytesIter {
            data: self.raw_sample_bytes(),
            repeat_length: self.repeat_length(),
            repeat_point: self.repeat_point(),
            position: 0,
        }
    }
}

/// Generates the 1 byte PCM samples contained within a sample.
///
/// This is infinite if the sample loops.
pub struct SampleBytesIter<'a> {
    /// Our sample, as bytes
    data: &'a [u8],
    /// The repeat point, in words
    repeat_point: u16,
    /// The repeat length, in words
    repeat_length: u16,
    /// Our current position, in bytes
    position: usize,
}

impl<'a> Iterator for SampleBytesIter<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        let sample = self.data.get(self.position).cloned();
        self.position += 1;
        if self.repeat_length != 1 {
            // this sample repeats
            if self.position >= usize::from(self.repeat_point + self.repeat_length) * 2 {
                self.position = usize::from(self.repeat_point) * 2;
            }
        }
        sample
    }
}

/// Iterates through all the samples in a module.
///
/// Generated by [`ProTrackerModule::samples()`].
pub struct SampleIter<'a> {
    parent: &'a ProTrackerModule<'a>,
    sample_no: u8,
    file_offset: usize,
}

impl<'a> core::iter::Iterator for SampleIter<'a> {
    type Item = Sample<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sample_no <= 31 {
            let sample = Sample::new(self.sample_no, self.file_offset, self.parent);
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
