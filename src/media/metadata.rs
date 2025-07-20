use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug)]
pub struct AudioMetadata {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct AiffData {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub audio_samples: Vec<i32>, // Raw samples in original bit depth
}

pub fn read_audio_metadata(path: &Path) -> Result<AudioMetadata, Box<dyn std::error::Error>> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("flac") => read_flac_metadata(path),
        Some("wav") => read_wav_metadata(path),
        _ => Err("Unsupported audio format".into()),
    }
}

fn read_flac_metadata(path: &Path) -> Result<AudioMetadata, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;

    if &magic != b"fLaC" {
        return Err("Not a valid FLAC file".into());
    }

    // Read METADATA_BLOCK_HEADER
    let mut header = [0u8; 4];
    file.read_exact(&mut header)?;

    // First bit indicates if this is the last metadata block
    // Next 7 bits are the block type (0 = STREAMINFO)
    // Next 24 bits are the block size
    let block_type = header[0] & 0x7F;
    if block_type != 0 {
        return Err("Expected STREAMINFO block".into());
    }

    // Read STREAMINFO
    let mut streaminfo = [0u8; 34]; // STREAMINFO is always 34 bytes
    file.read_exact(&mut streaminfo)?;

    // Extract fields from STREAMINFO
    // Skip min/max block size (4 bytes) and min/max frame size (6 bytes)
    let sample_rate = u32::from_be_bytes([0, streaminfo[10], streaminfo[11], streaminfo[12]]) >> 4;
    let channels = ((streaminfo[12] & 0x0E) >> 1) + 1;
    let bits_per_sample = (((streaminfo[12] & 0x01) << 4) | ((streaminfo[13] & 0xF0) >> 4)) + 1;

    // Total samples is a 36-bit value
    let total_samples = ((streaminfo[13] as u64 & 0x0F) << 32)
        | (streaminfo[14] as u64) << 24
        | (streaminfo[15] as u64) << 16
        | (streaminfo[16] as u64) << 8
        | (streaminfo[17] as u64);

    let duration_seconds = if sample_rate > 0 && total_samples > 0 {
        Some(total_samples as f64 / sample_rate as f64)
    } else {
        None
    };

    Ok(AudioMetadata {
        sample_rate,
        channels: channels as u16,
        bits_per_sample: bits_per_sample as u16,
        duration_seconds,
    })
}

fn read_wav_metadata(path: &Path) -> Result<AudioMetadata, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut riff = [0u8; 4];
    file.read_exact(&mut riff)?;

    if &riff != b"RIFF" {
        return Err("Not a valid WAV file".into());
    }

    // Skip file size
    file.seek(SeekFrom::Current(4))?;

    let mut wave = [0u8; 4];
    file.read_exact(&mut wave)?;
    if &wave != b"WAVE" {
        return Err("Not a valid WAV file".into());
    }

    // Find fmt chunk
    loop {
        let mut chunk_id = [0u8; 4];
        file.read_exact(&mut chunk_id)?;

        let mut chunk_size = [0u8; 4];
        file.read_exact(&mut chunk_size)?;
        let size = u32::from_le_bytes(chunk_size);

        if &chunk_id == b"fmt " {
            // Read format chunk
            let mut fmt_data = vec![0u8; size.min(16) as usize];
            file.read_exact(&mut fmt_data)?;

            let channels = u16::from_le_bytes([fmt_data[2], fmt_data[3]]);
            let sample_rate =
                u32::from_le_bytes([fmt_data[4], fmt_data[5], fmt_data[6], fmt_data[7]]);
            let bits_per_sample = u16::from_le_bytes([fmt_data[14], fmt_data[15]]);

            // Try to find data chunk for duration
            let mut duration_seconds = None;
            while let Ok(()) = file.read_exact(&mut chunk_id) {
                file.read_exact(&mut chunk_size)?;
                let data_size = u32::from_le_bytes(chunk_size);

                if &chunk_id == b"data" {
                    let bytes_per_sample = (bits_per_sample / 8) as u32;
                    let bytes_per_second = sample_rate * channels as u32 * bytes_per_sample;
                    if bytes_per_second > 0 {
                        duration_seconds = Some(data_size as f64 / bytes_per_second as f64);
                    }
                    break;
                } else {
                    file.seek(SeekFrom::Current(data_size as i64))?;
                }
            }

            return Ok(AudioMetadata {
                sample_rate,
                channels,
                bits_per_sample,
                duration_seconds,
            });
        } else {
            // Skip this chunk
            file.seek(SeekFrom::Current(size as i64))?;
        }
    }
}

#[allow(dead_code)]
fn read_aiff_metadata(path: &Path) -> Result<AudioMetadata, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;

    // Read FORM header
    let mut form = [0u8; 4];
    file.read_exact(&mut form)?;
    if &form != b"FORM" {
        return Err("Not a valid AIFF file".into());
    }

    // Skip file size (big-endian)
    file.seek(SeekFrom::Current(4))?;

    // Read AIFF identifier
    let mut aiff = [0u8; 4];
    file.read_exact(&mut aiff)?;
    if &aiff != b"AIFF" {
        return Err("Not a valid AIFF file".into());
    }

    // Find COMM chunk
    loop {
        let mut chunk_id = [0u8; 4];
        if file.read_exact(&mut chunk_id).is_err() {
            break;
        }

        let mut chunk_size = [0u8; 4];
        file.read_exact(&mut chunk_size)?;
        let size = u32::from_be_bytes(chunk_size); // Big-endian for AIFF

        if &chunk_id == b"COMM" {
            // Read COMM chunk (Common chunk)
            let mut comm_data = vec![0u8; size.min(18) as usize];
            file.read_exact(&mut comm_data)?;

            let channels = u16::from_be_bytes([comm_data[0], comm_data[1]]);
            let num_sample_frames =
                u32::from_be_bytes([comm_data[2], comm_data[3], comm_data[4], comm_data[5]]);
            let bits_per_sample = u16::from_be_bytes([comm_data[6], comm_data[7]]);

            // AIFF stores sample rate as 80-bit IEEE 754 extended precision
            // For simplicity, we'll extract the mantissa and exponent parts
            let sample_rate = if comm_data.len() >= 18 {
                // Quick approximation: use the first few bytes of the 80-bit float
                let exp = u16::from_be_bytes([comm_data[8], comm_data[9]]);
                let mantissa = u64::from_be_bytes([
                    0,
                    0,
                    comm_data[10],
                    comm_data[11],
                    comm_data[12],
                    comm_data[13],
                    comm_data[14],
                    comm_data[15],
                ]);

                // Rough conversion from 80-bit IEEE 754 extended
                if exp == 0 {
                    0
                } else {
                    let real_exp = (exp & 0x7FFF) as i32 - 16383;
                    let mantissa_f64 = mantissa as f64 / (1u64 << 48) as f64;
                    ((1.0 + mantissa_f64) * 2.0f64.powi(real_exp)) as u32
                }
            } else {
                44100 // Fallback
            };

            // Calculate duration from number of sample frames
            let duration_seconds = if sample_rate > 0 {
                Some(num_sample_frames as f64 / sample_rate as f64)
            } else {
                None
            };

            return Ok(AudioMetadata {
                sample_rate,
                channels,
                bits_per_sample,
                duration_seconds,
            });
        } else {
            // Skip this chunk
            file.seek(SeekFrom::Current(size as i64))?;
        }
    }

    Err("COMM chunk not found in AIFF file".into())
}

#[allow(dead_code)]
pub fn read_aiff_data(path: &Path) -> Result<AiffData, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;

    // Read FORM header
    let mut form = [0u8; 4];
    file.read_exact(&mut form)?;
    if &form != b"FORM" {
        return Err("Not a valid AIFF file".into());
    }

    // Skip file size (big-endian)
    file.seek(SeekFrom::Current(4))?;

    // Read AIFF identifier
    let mut aiff = [0u8; 4];
    file.read_exact(&mut aiff)?;
    if &aiff != b"AIFF" {
        return Err("Not a valid AIFF file".into());
    }

    let mut sample_rate = 0;
    let mut channels = 0;
    let mut bits_per_sample = 0;
    let mut audio_samples = Vec::new();

    // Find both COMM and SSND chunks
    loop {
        let mut chunk_id = [0u8; 4];
        if file.read_exact(&mut chunk_id).is_err() {
            break;
        }

        let mut chunk_size = [0u8; 4];
        file.read_exact(&mut chunk_size)?;
        let size = u32::from_be_bytes(chunk_size); // Big-endian for AIFF

        if &chunk_id == b"COMM" {
            // Read COMM chunk (Common chunk)
            let mut comm_data = vec![0u8; size.min(18) as usize];
            file.read_exact(&mut comm_data)?;

            channels = u16::from_be_bytes([comm_data[0], comm_data[1]]);
            let _num_sample_frames =
                u32::from_be_bytes([comm_data[2], comm_data[3], comm_data[4], comm_data[5]]);
            bits_per_sample = u16::from_be_bytes([comm_data[6], comm_data[7]]);

            // AIFF stores sample rate as 80-bit IEEE 754 extended precision
            // For simplicity, we'll extract the mantissa and exponent parts
            sample_rate = if comm_data.len() >= 18 {
                // Quick approximation: use the first few bytes of the 80-bit float
                let exp = u16::from_be_bytes([comm_data[8], comm_data[9]]);
                let mantissa = u64::from_be_bytes([
                    0,
                    0,
                    comm_data[10],
                    comm_data[11],
                    comm_data[12],
                    comm_data[13],
                    comm_data[14],
                    comm_data[15],
                ]);

                // Rough conversion from 80-bit IEEE 754 extended
                if exp == 0 {
                    0
                } else {
                    let real_exp = (exp & 0x7FFF) as i32 - 16383;
                    let mantissa_f64 = mantissa as f64 / (1u64 << 48) as f64;
                    ((1.0 + mantissa_f64) * 2.0f64.powi(real_exp)) as u32
                }
            } else {
                44100 // Fallback
            };
        } else if &chunk_id == b"SSND" {
            // Read SSND chunk (Sound Data chunk)
            // SSND has an 8-byte header: offset (4) + blockSize (4)
            let mut ssnd_header = [0u8; 8];
            file.read_exact(&mut ssnd_header)?;

            let offset = u32::from_be_bytes([
                ssnd_header[0],
                ssnd_header[1],
                ssnd_header[2],
                ssnd_header[3],
            ]);
            let _block_size = u32::from_be_bytes([
                ssnd_header[4],
                ssnd_header[5],
                ssnd_header[6],
                ssnd_header[7],
            ]);

            // Skip offset bytes if any
            if offset > 0 {
                file.seek(SeekFrom::Current(offset as i64))?;
            }

            // Calculate how many audio bytes to read
            let audio_bytes = size - 8 - offset; // size minus header minus offset

            // Read audio data based on bit depth
            match bits_per_sample {
                8 => {
                    // 8-bit samples (signed)
                    let mut bytes = vec![0u8; audio_bytes as usize];
                    file.read_exact(&mut bytes)?;
                    audio_samples = bytes.into_iter().map(|b| (b as i8) as i32).collect();
                }
                16 => {
                    // 16-bit samples (big-endian)
                    let sample_count = (audio_bytes / 2) as usize;
                    audio_samples.reserve(sample_count);
                    for _ in 0..sample_count {
                        let mut sample_bytes = [0u8; 2];
                        file.read_exact(&mut sample_bytes)?;
                        let sample = i16::from_be_bytes(sample_bytes);
                        audio_samples.push(sample as i32);
                    }
                }
                24 => {
                    // 24-bit samples (big-endian, sign-extended to 32-bit)
                    let sample_count = (audio_bytes / 3) as usize;
                    audio_samples.reserve(sample_count);
                    for _ in 0..sample_count {
                        let mut sample_bytes = [0u8; 3];
                        file.read_exact(&mut sample_bytes)?;
                        // Convert 24-bit big-endian to 32-bit signed
                        let sample = ((sample_bytes[0] as i32) << 16)
                            | ((sample_bytes[1] as i32) << 8)
                            | (sample_bytes[2] as i32);
                        // Sign extend from 24-bit to 32-bit
                        let sample = if sample & 0x800000 != 0 {
                            sample | 0xFF000000u32 as i32
                        } else {
                            sample
                        };
                        audio_samples.push(sample);
                    }
                }
                32 => {
                    // 32-bit samples (big-endian)
                    let sample_count = (audio_bytes / 4) as usize;
                    audio_samples.reserve(sample_count);
                    for _ in 0..sample_count {
                        let mut sample_bytes = [0u8; 4];
                        file.read_exact(&mut sample_bytes)?;
                        let sample = i32::from_be_bytes(sample_bytes);
                        audio_samples.push(sample);
                    }
                }
                _ => return Err(format!("Unsupported bit depth: {bits_per_sample}").into()),
            }
        } else {
            // Skip this chunk
            file.seek(SeekFrom::Current(size as i64))?;
        }
    }

    if sample_rate == 0 || channels == 0 || bits_per_sample == 0 {
        return Err("Missing COMM chunk in AIFF file".into());
    }

    if audio_samples.is_empty() {
        return Err("Missing SSND chunk in AIFF file".into());
    }

    Ok(AiffData {
        sample_rate,
        channels,
        bits_per_sample,
        audio_samples,
    })
}
