# Audio Mixing Guide for `zim play`

## Quick Start

Mix up to 3 audio files simultaneously for auditioning:

```bash
# Basic mixing (equal volume)
zim play drums.wav bass.wav vocals.wav

# With custom gain levels
zim play drums.wav bass.wav vocals.wav --gains 0.8,1.2,0.6
```

## Gain Values Reference

The `--gains` flag accepts comma-separated values from 0.0 to 2.0:
- `1.0` = Unity gain (original volume)
- `0.5` = Half volume (-6dB)
- `0.0` = Mute
- `2.0` = Double volume (+6dB, may cause clipping)

## Common Mixing Scenarios

### Two Files
```bash
# Balanced mix
zim play track1.wav track2.wav --gains 1.0,1.0

# Emphasize first track
zim play track1.wav track2.wav --gains 1.2,0.8

# Background music with voiceover
zim play music.wav voice.wav --gains 0.4,1.0
```

### Three Files
```bash
# Balanced mix with headroom
zim play drums.wav bass.wav guitar.wav --gains 0.7,0.7,0.7

# Rock mix (drums and bass foundation, guitar on top)
zim play drums.wav bass.wav guitar.wav --gains 0.8,1.0,0.6

# Electronic mix (kick prominent, bass supporting, lead melody)
zim play kick.wav bass.wav synth.wav --gains 1.2,0.8,0.6
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