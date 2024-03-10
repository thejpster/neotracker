//! Extract a sample from a mod file
//! 
//! Saves it as raw 8-bit signed samples, and loops it out to 3 seconds long at as a C3.

fn main() {
    let filename = std::env::args_os().nth(1).expect("filename");
    let sample_no = std::env::args().nth(2).expect("sample number");
    let sample_no = sample_no.parse::<u8>().expect("integer sample number");
    let out_file = std::env::args_os().nth(3).expect("output file name");
    let data = std::fs::read(filename).expect("open file");
    let ptm = neotracker::ProTrackerModule::new(&data).expect("supported mod file");
    let sample = ptm.sample(sample_no).expect("sample should exist");
    let sample_data = sample.sample_bytes_iter().take(16754 * 3).collect::<Vec<u8>>();
    if !sample_data.is_empty() {
        std::fs::write(&out_file, &sample_data).expect("write sample file");
        println!(
            "Wrote {} bytes looped to {} bytes for sample {} to {}",
            sample.sample_length_bytes(),
            sample_data.len(),
            sample_no,
            out_file.to_string_lossy()
        );    
    }
}
