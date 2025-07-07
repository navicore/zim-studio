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
