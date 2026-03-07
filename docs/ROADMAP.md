# Roadmap

## Current State (v1.4.2)

ZIM Studio is functional and actively used. Core features are stable:
- Project scaffolding, sidecar generation, YAML linting, index generation, metadata sync
- WAV metadata embedding with UUID lineage tracking
- TUI player with mark/loop/save, multi-file mixing, fuzzy browser, oscilloscope

## Known In-Progress

- **AIFF support**: Listed as WIP in README. WAV and FLAC are supported; AIFF decoding is not yet implemented.

## Prior Refactoring

The `docs/refactor/` directory contains analysis notes from a previous refactoring effort across 7 phases covering app, audio, browser, UI, lint, update, and smaller modules.

## Potential Next Steps

These are inferred from code and README — not confirmed priorities:
- AIFF format support completion
- Additional audio format support beyond WAV/FLAC/AIFF
