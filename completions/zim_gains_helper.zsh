#!/usr/bin/env zsh
# Helper completion for zim play --gains flag

# Function to suggest gain values based on number of files
_zim_suggest_gains() {
    local -a files
    local gains_suggestion
    
    # Count audio files in the current command line
    files=(${(z)words})
    local file_count=0
    
    for word in $files; do
        if [[ $word == *.wav || $word == *.flac || $word == *.aif || $word == *.aiff ]]; then
            ((file_count++))
        fi
    done
    
    # Provide gain suggestions based on file count
    case $file_count in
        1)
            _values 'gain values' \
                '1.0[Unity gain (default)]' \
                '0.5[Half volume]' \
                '0.8[Slightly quieter]' \
                '1.2[Slightly louder]' \
                '1.5[50% louder]' \
                '2.0[Maximum gain]'
            ;;
        2)
            _values 'gain values for 2 files' \
                '1.0,1.0[Both at unity gain]' \
                '0.8,1.2[First quieter, second louder]' \
                '1.2,0.8[First louder, second quieter]' \
                '0.7,0.7[Both at 70% for headroom]' \
                '1.0,0.5[Second at half volume]'
            ;;
        3)
            _values 'gain values for 3 files' \
                '1.0,1.0,1.0[All at unity gain]' \
                '0.8,1.0,0.6[Balanced mix]' \
                '1.0,0.8,0.6[Graduated levels]' \
                '0.6,0.6,0.6[All at 60% for headroom]' \
                '1.2,1.0,0.8[Emphasize first file]'
            ;;
        *)
            _values 'gain values' \
                '1.0[Unity gain]' \
                '0.8,1.2[Two files: first quieter]' \
                '0.8,1.0,0.6[Three files: balanced]'
            ;;
    esac
}

# Add this to your .zshrc or source it:
# This hooks into zim's completion for the --gains flag
if [[ -n ${ZSH_VERSION-} ]]; then
    # Override completion for --gains when it appears
    compdef '_zim_suggest_gains' zim
fi