//! This test file checks we can parse the file "cd_axelf.mod". I pretty much
//! chose it at random.

/// A test mod file
static DATA: &[u8] = include_bytes!("cd_axelf.mod");

#[test]
fn recognise_file() {
    let _pt = neotracker::ProTrackerModule::new(DATA).unwrap();
}

#[test]
fn sample_names() {
    static SAMPLE_NAMES: &[&[u8]] = &[
        // Sample 0
        b"brazzstring1",
        // Sample 1
        b"brazzstring2",
        // Sample 2
        b"brazzstring3",
        // Sample 3
        b"brazzstring5",
        // Sample 4
        b"MixSynth3.bas",
        // Sample 5
        b"49-Bass5",
        // Sample 6
        b"BuffBD",
        // Sample 7
        b"snare25",
        // Sample 8
        b"efg-string.1",
        // Sample 9
        b"efg-string.2",
        // Sample 10
        b"efg-string.3",
        // Sample 11
        b"hihat-open",
        // Sample 12
        b"",
        // Sample 13
        b"vogue-pianochord.5",
        // Sample 14
        b"vogue-pianochord.4",
        // Sample 15
        b"vogue-pianochord.7",
        // Sample 16
        b"squarewave",
        // Sample 17
        b"mt.lead-2494-7e",
        // Sample 18
        b"",
        // Sample 19
        b"",
        // Sample 20
        b"",
        // Sample 21
        b"",
        // Sample 22
        b"",
        // Sample 23
        b"",
        // Sample 24
        b"",
        // Sample 25
        b"",
        // Sample 26
        b"",
        // Sample 27
        b"",
        // Sample 28
        b"",
        // Sample 29
        b"",
        // Sample 30
        b"",
    ];

    let pt = neotracker::ProTrackerModule::new(DATA).unwrap();

    for (idx, sample) in pt.samples().enumerate() {
        assert_eq!(
            sample.name(),
            SAMPLE_NAMES[idx],
            "Sample {} name is wrong {:?} != {:?}",
            idx,
            std::str::from_utf8(sample.name()),
            std::str::from_utf8(SAMPLE_NAMES[idx])
        );
    }
}

#[test]
fn sample_properties() {
    static SAMPLE_PROPERTIES: &[(usize, u8, u8, usize, usize)] = &[
        // Sample 0
        (0, 0, 0, 0, 0),
        // Sample 1
        (0, 0, 0, 0, 0),
        // Sample 2
        (0, 0, 0, 0, 0),
        // Sample 3
        (0, 0, 0, 0, 0),
        // Sample 4
        (3000, 0, 52, 0, 2),
        // Sample 5
        (9692, 0, 64, 0, 2),
        // Sample 6
        (4076, 0, 47, 0, 2),
        // Sample 7
        (3010, 0, 64, 0, 2),
        // Sample 8
        (24060, 0, 64, 7126, 16934),
        // Sample 9
        (28922, 0, 64, 10592, 18330),
        // Sample 10
        (29166, 0, 64, 8544, 20622),
        // Sample 11
        (9938, 0, 24, 0, 2),
        // Sample 12
        (0, 0, 0, 0, 2),
        // Sample 13
        (8554, 0, 64, 0, 2),
        // Sample 14
        (6324, 0, 64, 0, 2),
        // Sample 15
        (8442, 0, 64, 0, 2),
        // Sample 16
        (14056, 0, 44, 4430, 9626),
        // Sample 17
        (9490, 0, 51, 0, 9490),
        // Sample 18
        (0, 0, 0, 0, 2),
        // Sample 19
        (0, 0, 0, 0, 2),
        // Sample 20
        (0, 0, 0, 0, 2),
        // Sample 21
        (0, 0, 0, 0, 2),
        // Sample 22
        (0, 0, 0, 0, 2),
        // Sample 23
        (0, 0, 0, 0, 2),
        // Sample 24
        (0, 0, 0, 0, 2),
        // Sample 25
        (0, 0, 0, 0, 2),
        // Sample 26
        (0, 0, 0, 0, 2),
        // Sample 27
        (0, 0, 0, 0, 2),
        // Sample 28
        (0, 0, 0, 0, 2),
        // Sample 29
        (0, 0, 0, 0, 2),
        // Sample 30
        (0, 0, 0, 0, 2),
    ];

    let pt = neotracker::ProTrackerModule::new(DATA).unwrap();

    for (idx, sample) in pt.samples().enumerate() {
        // sample_length_bytes
        // finetune
        // volume
        // repeat_point_bytes
        // repeat_length_bytes
        let info = (
            sample.sample_length_bytes(),
            sample.finetune(),
            sample.volume(),
            sample.repeat_point_bytes(),
            sample.repeat_length_bytes(),
        );
        assert_eq!(
            info, SAMPLE_PROPERTIES[idx],
            "Sample {} info is wrong {:?} != {:?}",
            idx, info, SAMPLE_PROPERTIES[idx],
        );
    }
}

#[test]
fn decode_song() {
    use std::fmt::Write;
    // This file was generated with "genpattern", which uses an alternative mod parser
    let expected = include_str!("cd_axelf.txt");
    let mut buffer = String::new();
    let pt = neotracker::ProTrackerModule::new(DATA).unwrap();

    for (idx, si) in pt.samples().enumerate() {
        writeln!(
            buffer,
            "Sample {}: {} {} {} {} {}",
            idx,
            si.sample_length(),
            si.finetune(),
            si.volume(),
            si.repeat_point(),
            si.repeat_length()
        )
        .unwrap();
    }

    for pattern in pt.song_positions() {
        writeln!(buffer, "Pattern {}", pattern).unwrap();
        let pattern = pt.pattern(*pattern).unwrap();
        for line in pattern.lines() {
            write!(buffer, "\t|").unwrap();
            for ch in 0..4 {
                write!(
                    buffer,
                    " {:02x} {:06} {:04x} |",
                    line.channel[ch].sample_no(),
                    line.channel[ch].period(),
                    line.channel[ch].effect_u16(),
                )
                .unwrap();
            }
            writeln!(buffer).unwrap();
        }
    }
    assert_eq!(expected, buffer);
}
