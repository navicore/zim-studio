# Audio Mixing Guide for `zim play`

## Quick Start

Mix up to 3 audio files simultaneously for auditioning:

```bash
# Basic mixing (equal volume, centered)
zim play drums.wav bass.wav vocals.wav

# With custom gain levels
zim play drums.wav bass.wav vocals.wav --gains 0.8,1.2,0.6

# With panning for stereo field
zim play drums.wav bass.wav vocals.wav --pans 0.0,-0.5,0.8

# Both gain and pan control
zim play drums.wav bass.wav vocals.wav --gains 0.8,1.2,0.6 --pans 0.0,-0.5,0.8
```

## Control References

### Gain Values (`--gains`)
The `--gains` flag accepts comma-separated values from 0.0 to 2.0:
- `1.0` = Unity gain (original volume)
- `0.5` = Half volume (-6dB)
- `0.0` = Mute
- `2.0` = Double volume (+6dB, may cause clipping)

### Pan Values (`--pans`)
The `--pans` flag accepts comma-separated values from -1.0 to 1.0:
- `-1.0` = Hard left
- `-0.5` = Left of center
- `0.0` = Center (default)
- `0.5` = Right of center
- `1.0` = Hard right

## Common Mixing Scenarios

### Two Files
```bash
# Balanced stereo mix
zim play track1.wav track2.wav --gains 1.0,1.0 --pans -0.3,0.3

# Left-right separation 
zim play left.wav right.wav --pans -1.0,1.0

# Background music with centered voice
zim play music.wav voice.wav --gains 0.4,1.0 --pans -0.2,0.0

# Fix stereo imbalance
zim play track.wav track.wav --gains 1.0,0.8 --pans -0.1,0.1
```

### Three Files
```bash
# Classic stereo spread (drums center, bass left, guitar right)
zim play drums.wav bass.wav guitar.wav --gains 0.8,0.7,0.7 --pans 0.0,-0.6,0.6

# Wide stereo mix with centered focal point
zim play wide1.wav vocal.wav wide2.wav --gains 0.6,1.0,0.6 --pans -0.8,0.0,0.8

# Corrected imbalance (shift everything slightly right)
zim play track1.wav track2.wav track3.wav --pans 0.1,0.2,0.15
```

## Tips for Gain Settings

1. **Prevent Clipping**: When mixing multiple files, consider reducing all gains proportionally:
   - 2 files: Try `0.7,0.7` instead of `1.0,1.0`
   - 3 files: Try `0.6,0.6,0.6` for more headroom

2. **Quick Audition Patterns**:
   - Solo check: Set others to 0.0 (e.g., `1.0,0.0,0.0`)
   - A/B comparison: Alternate between `1.0,0.0` and `0.0,1.0`
   - Gradual blend: Use `1.0,0.7,0.4` for layered depth

3. **Shell Completion**: After typing `--gains `, press TAB to see common suggestions (requires zsh completions installed)

## Exporting Mixed Audio

Once you've found the perfect mix:
1. Use the player's mark in/out feature (`[` and `]` keys)
2. Press `s` to save the mixed selection
3. The exported file will contain your mixed audio at the specified gain levels