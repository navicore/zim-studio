//! Minimal WAV file metadata manipulation for tracking file lineage
//!
//! This module provides functionality to read and write INFO LIST chunks
//! in WAV files without external dependencies.

use chrono::Utc;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use uuid::Uuid;

/// Core metadata to embed in WAV files
#[derive(Debug, Clone)]
pub struct ZimMetadata {
    // Identity
    pub uuid: String,
    pub parent_uuid: Option<String>,

    // Origin tracking
    pub project: String,
    pub first_seen: String, // ISO 8601 timestamp
    pub original_path: String,

    // Lineage
    pub generation: u32,
    pub transform: Option<String>, // "excerpt", "mix", "bounce", etc.

    // Fingerprint
    pub audio_md5: String,

    // Tool info
    pub zim_version: String,
}

impl ZimMetadata {
    /// Create metadata for an original file (first time zim sees it)
    pub fn new_original(project: &str, path: &Path) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            parent_uuid: None,
            project: project.to_string(),
            first_seen: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            original_path: path.to_string_lossy().to_string(),
            generation: 0,
            transform: None,
            audio_md5: String::new(), // Will be calculated
            zim_version: format!("zim-studio-v{}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Create metadata for a derived file (excerpt, mix, etc.)
    pub fn new_derived(&self, transform: &str) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            parent_uuid: Some(self.uuid.clone()),
            project: self.project.clone(),
            first_seen: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            original_path: self.original_path.clone(), // Preserve original path by default
            generation: self.generation + 1,
            transform: Some(transform.to_string()),
            audio_md5: String::new(), // Will be calculated
            zim_version: self.zim_version.clone(),
        }
    }
}

/// Read a 4-byte chunk ID
fn read_fourcc(reader: &mut impl Read) -> Result<String, Box<dyn Error>> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Read a 4-byte little-endian size
fn read_u32_le(reader: &mut impl Read) -> Result<u32, Box<dyn Error>> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

/// Write a 4-byte little-endian size
fn write_u32_le(writer: &mut impl Write, value: u32) -> Result<(), Box<dyn Error>> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

/// Calculate MD5 of audio data in a WAV file
pub fn calculate_audio_md5(path: &Path) -> Result<String, Box<dyn Error>> {
    // Security: Check file size before processing
    const MAX_WAV_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4GB max
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_WAV_SIZE {
        return Err(format!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            MAX_WAV_SIZE
        )
        .into());
    }

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    // Read RIFF header
    let riff_id = read_fourcc(&mut reader)?;
    if riff_id != "RIFF" {
        return Err("Not a RIFF file".into());
    }

    let file_size = read_u32_le(&mut reader)?;

    // Security: Validate file size matches actual file
    if file_size as u64 + 8 != metadata.len() {
        return Err("Invalid RIFF size field".into());
    }

    let wave_id = read_fourcc(&mut reader)?;
    if wave_id != "WAVE" {
        return Err("Not a WAVE file".into());
    }

    // Track position to prevent reading past file size
    let mut pos = 12u64; // Already read 12 bytes (RIFF + size + WAVE)
    let max_pos = file_size as u64 + 8; // RIFF size doesn't include RIFF header

    // Find data chunk
    while pos < max_pos {
        let chunk_id = match read_fourcc(&mut reader) {
            Ok(id) => id,
            Err(_) => break, // EOF reached
        };
        let chunk_size = match read_u32_le(&mut reader) {
            Ok(size) => size,
            Err(_) => break, // EOF reached
        };
        pos += 8;

        // Security: Validate chunk size more thoroughly
        if chunk_size > file_size {
            return Err("Invalid chunk size: exceeds file size".into());
        }
        if chunk_size as u64 > max_pos - pos {
            return Err("Invalid chunk size: exceeds remaining file space".into());
        }

        if chunk_id == "data" {
            // Security: Limit data chunk size to prevent excessive memory use
            const MAX_DATA_CHUNK: u32 = 2 * 1024 * 1024 * 1024; // 2GB max data
            if chunk_size > MAX_DATA_CHUNK {
                return Err(format!("Data chunk too large: {chunk_size} bytes").into());
            }

            // Calculate MD5 using chunked reading for large files
            let mut context = md5::Context::new();

            const BUFFER_SIZE: usize = 8192; // 8KB chunks
            let mut buffer = vec![0u8; BUFFER_SIZE];
            let mut remaining = chunk_size as usize;

            while remaining > 0 {
                let to_read = remaining.min(BUFFER_SIZE);
                let bytes_read = reader.read(&mut buffer[..to_read])?;
                if bytes_read == 0 {
                    return Err("Unexpected end of file in data chunk".into());
                }
                context.consume(&buffer[..bytes_read]);
                remaining -= bytes_read;
            }

            let digest = context.finalize();
            return Ok(format!("{digest:x}"));
        } else {
            // Skip this chunk
            reader.seek(SeekFrom::Current(chunk_size as i64))?;
            pos += chunk_size as u64;
            // Pad byte if chunk size is odd
            if chunk_size % 2 == 1 {
                reader.seek(SeekFrom::Current(1))?;
                pos += 1;
            }
        }
    }

    Err("Data chunk not found".into())
}

/// Read ZIM metadata from WAV file's INFO chunk
pub fn read_metadata(path: &Path) -> Result<Option<ZimMetadata>, Box<dyn Error>> {
    // Security: Check file size before processing
    const MAX_WAV_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4GB max
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_WAV_SIZE {
        return Err(format!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            MAX_WAV_SIZE
        )
        .into());
    }

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    // Read RIFF header
    let riff_id = read_fourcc(&mut reader)?;
    if riff_id != "RIFF" {
        return Err("Not a RIFF file".into());
    }

    let file_size = read_u32_le(&mut reader)?;
    let wave_id = read_fourcc(&mut reader)?;
    if wave_id != "WAVE" {
        return Err("Not a WAVE file".into());
    }

    // Track position to prevent reading past file size
    let mut pos = 12u64; // Already read 12 bytes (RIFF + size + WAVE)
    let max_pos = file_size as u64 + 8; // RIFF size doesn't include RIFF header

    // Look for LIST INFO chunk
    while pos < max_pos {
        let chunk_id = match read_fourcc(&mut reader) {
            Ok(id) => id,
            Err(_) => return Ok(None), // End of file
        };
        let chunk_size = match read_u32_le(&mut reader) {
            Ok(size) => size,
            Err(_) => return Ok(None), // End of file
        };
        pos += 8;

        // Validate chunk size
        if chunk_size > file_size {
            return Err("Invalid chunk size".into());
        }

        if chunk_id == "LIST" && chunk_size >= 4 {
            let list_type = read_fourcc(&mut reader)?;
            if list_type == "INFO" {
                // Parse INFO chunk
                return parse_info_chunk(&mut reader, chunk_size - 4);
            }
            // Skip the rest of this LIST chunk
            reader.seek(SeekFrom::Current((chunk_size - 4) as i64))?;
            pos += chunk_size as u64;
        } else {
            // Skip this chunk
            reader.seek(SeekFrom::Current(chunk_size as i64))?;
            pos += chunk_size as u64;
        }

        // Pad byte if chunk size is odd
        if chunk_size % 2 == 1 {
            reader.seek(SeekFrom::Current(1))?;
            pos += 1;
        }
    }

    Ok(None) // No INFO chunk found
}

/// Parse INFO chunk to extract ZIM metadata
fn parse_info_chunk(
    reader: &mut impl Read,
    size: u32,
) -> Result<Option<ZimMetadata>, Box<dyn Error>> {
    // Security: Limit INFO chunk size
    const MAX_INFO_SIZE: u32 = 1024 * 1024; // 1MB max for INFO chunk
    if size > MAX_INFO_SIZE {
        return Err(format!("INFO chunk too large: {size} bytes").into());
    }

    let mut bytes_read = 0u32;
    let mut zim_data = String::new();

    while bytes_read < size {
        let field_id = read_fourcc(reader)?;
        let field_size = read_u32_le(reader)?;

        // Security: Validate field size
        if field_size > size - bytes_read {
            return Err("Invalid INFO field size".into());
        }
        if field_size > 65536 {
            // 64KB max per field
            return Err(format!("INFO field too large: {field_size} bytes").into());
        }

        let mut field_data = vec![0u8; field_size as usize];
        reader.read_exact(&mut field_data)?;

        let field_str = String::from_utf8_lossy(&field_data);

        // Look for our special ICMT field with ZIM data
        if field_id == "ICMT" && field_str.starts_with("ZIM:") {
            zim_data = field_str[4..].to_string();
        }

        bytes_read += 8 + field_size;

        // Pad byte if field size is odd
        if field_size % 2 == 1 {
            let mut pad = [0u8; 1];
            reader.read_exact(&mut pad)?;
            bytes_read += 1;
        }
    }

    // Parse ZIM data (simple key=value format)
    if !zim_data.is_empty() {
        let mut metadata = ZimMetadata::new_original("unknown", Path::new(""));

        for part in zim_data.split(';') {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "uuid" => metadata.uuid = value.to_string(),
                    "parent" => metadata.parent_uuid = Some(value.to_string()),
                    "project" => metadata.project = value.to_string(),
                    "gen" => metadata.generation = value.parse().unwrap_or(0),
                    "transform" => metadata.transform = Some(value.to_string()),
                    "md5" => metadata.audio_md5 = value.to_string(),
                    "path" => metadata.original_path = value.to_string(),
                    "first_seen" => metadata.first_seen = value.to_string(),
                    _ => {}
                }
            }
        }

        return Ok(Some(metadata));
    }

    Ok(None)
}

/// Write ZIM metadata to a WAV file (creates a new file)
pub fn write_metadata(
    input_path: &Path,
    output_path: &Path,
    metadata: &ZimMetadata,
) -> Result<(), Box<dyn Error>> {
    // Read entire input file
    let mut input_file = File::open(input_path)?;
    let mut wav_data = Vec::new();
    input_file.read_to_end(&mut wav_data)?;

    // Verify it's a RIFF WAVE file
    if &wav_data[0..4] != b"RIFF" || &wav_data[8..12] != b"WAVE" {
        return Err("Not a valid WAV file".into());
    }

    // Create INFO LIST chunk
    let info_chunk = create_info_chunk(metadata)?;

    // Find and remove existing INFO chunks, and find data chunk position
    let mut pos = 12; // After RIFF/size/WAVE
    let mut data_chunk_pos = None;
    let mut chunks_before_data = Vec::new();

    while pos < wav_data.len() - 8 {
        let chunk_id = &wav_data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes([
            wav_data[pos + 4],
            wav_data[pos + 5],
            wav_data[pos + 6],
            wav_data[pos + 7],
        ]);

        if chunk_id == b"data" {
            data_chunk_pos = Some(pos);
            break;
        } else if chunk_id == b"LIST" {
            // Check if it's an INFO LIST
            if pos + 12 <= wav_data.len() && &wav_data[pos + 8..pos + 12] == b"INFO" {
                // Skip this INFO chunk (don't add to chunks_before_data)
            } else {
                // Keep non-INFO LIST chunks
                let chunk_end = pos + 8 + chunk_size as usize;
                let padded_end = if chunk_size % 2 == 1 {
                    chunk_end + 1
                } else {
                    chunk_end
                };
                chunks_before_data.extend_from_slice(&wav_data[pos..padded_end]);
            }
        } else {
            // Keep all other chunks
            let chunk_end = pos + 8 + chunk_size as usize;
            let padded_end = if chunk_size % 2 == 1 {
                chunk_end + 1
            } else {
                chunk_end
            };
            chunks_before_data.extend_from_slice(&wav_data[pos..padded_end]);
        }

        // Skip to next chunk
        pos += 8 + chunk_size as usize;
        if chunk_size % 2 == 1 {
            pos += 1; // Pad byte
        }
    }

    // Build output file
    let output_file = File::create(output_path)?;
    let mut writer = BufWriter::new(output_file);

    // Write RIFF header
    writer.write_all(b"RIFF")?;

    // Calculate new file size (4 for "WAVE" + filtered chunks + INFO chunk + data chunk)
    let data_chunk = if let Some(data_pos) = data_chunk_pos {
        &wav_data[data_pos..]
    } else {
        &[]
    };
    let new_size = 4 + chunks_before_data.len() + info_chunk.len() + data_chunk.len();
    write_u32_le(&mut writer, new_size as u32)?;

    writer.write_all(b"WAVE")?;

    // Write non-INFO chunks that come before data
    writer.write_all(&chunks_before_data)?;

    // Write our new INFO chunk
    writer.write_all(&info_chunk)?;

    // Write data chunk and any chunks after it
    if !data_chunk.is_empty() {
        writer.write_all(data_chunk)?;
    }

    writer.flush()?;
    Ok(())
}

/// Create INFO LIST chunk with ZIM metadata
fn create_info_chunk(metadata: &ZimMetadata) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut chunk = Vec::new();

    // Build ZIM data string
    let mut zim_data = format!("ZIM:uuid={}", metadata.uuid);
    if let Some(parent) = &metadata.parent_uuid {
        zim_data.push_str(&format!(";parent={parent}"));
    }
    zim_data.push_str(&format!(";project={}", metadata.project));
    zim_data.push_str(&format!(";gen={}", metadata.generation));
    if let Some(transform) = &metadata.transform {
        zim_data.push_str(&format!(";transform={transform}"));
    }
    if !metadata.audio_md5.is_empty() {
        zim_data.push_str(&format!(";md5={}", metadata.audio_md5));
    }
    if !metadata.original_path.is_empty() {
        zim_data.push_str(&format!(";path={}", metadata.original_path));
    }
    if !metadata.first_seen.is_empty() {
        zim_data.push_str(&format!(";first_seen={}", metadata.first_seen));
    }

    // Also add human-readable fields
    let software = metadata.zim_version.to_string();

    // Build INFO sub-chunks
    let mut info_data = Vec::new();

    // ISFT (Software)
    info_data.extend(b"ISFT");
    let software_bytes = software.as_bytes();
    info_data.extend(&(software_bytes.len() as u32).to_le_bytes());
    info_data.extend(software_bytes);
    if software_bytes.len() % 2 == 1 {
        info_data.push(0); // Pad byte
    }

    // ICMT (Comment with ZIM data)
    info_data.extend(b"ICMT");
    let zim_bytes = zim_data.as_bytes();
    info_data.extend(&(zim_bytes.len() as u32).to_le_bytes());
    info_data.extend(zim_bytes);
    if zim_bytes.len() % 2 == 1 {
        info_data.push(0); // Pad byte
    }

    // Build LIST chunk
    chunk.extend(b"LIST");
    chunk.extend(&((info_data.len() + 4) as u32).to_le_bytes());
    chunk.extend(b"INFO");
    chunk.extend(info_data);

    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_metadata_roundtrip() {
        let dir = tempdir().unwrap();
        let input_wav = dir.path().join("test.wav");
        let output_wav = dir.path().join("test_tagged.wav");

        // Create a minimal WAV file
        create_test_wav(&input_wav);

        // Create metadata
        let mut metadata = ZimMetadata::new_original("test-project", &input_wav);
        metadata.audio_md5 = calculate_audio_md5(&input_wav).unwrap();

        // Write metadata
        write_metadata(&input_wav, &output_wav, &metadata).unwrap();

        // Read it back
        let read_metadata = read_metadata(&output_wav).unwrap().unwrap();

        assert_eq!(read_metadata.uuid, metadata.uuid);
        assert_eq!(read_metadata.project, "test-project");
        assert_eq!(read_metadata.generation, 0);
        assert_eq!(read_metadata.audio_md5, metadata.audio_md5);
    }

    fn create_test_wav(path: &Path) {
        use hound;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..100 {
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
    }
}
