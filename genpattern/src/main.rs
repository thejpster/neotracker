fn main() {
    let filename = std::env::args_os().nth(1).expect("filename");
    let mut f = std::fs::File::open(filename).expect("open file");
    let mf = modfile::ptmf::read_mod(&mut f, false).unwrap();

    for (idx, si) in mf.sample_info.iter().enumerate() {
        println!(
            "Sample {}: {} {} {} {} {}",
            idx, si.length, si.finetune, si.volume, si.repeat_start, si.repeat_length
        );
    }

    for pattern in mf.positions.data.iter().take(mf.length as usize) {
        println!("Pattern {}", *pattern);
        for row in mf.patterns[*pattern as usize].rows.iter() {
            print!("\t|");
            for ch in row.channels.iter() {
                print!(
                    " {:02x} {:06} {:04x} |",
                    ch.sample_number, ch.period, ch.effect,
                );
            }
            println!();
        }
    }
}
