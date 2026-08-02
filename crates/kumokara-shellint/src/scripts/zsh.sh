# zsh shell integration for Kumokara
# Injects OSC 133 (FTCS) and OSC 7 sequences for terminal observability.
# Phase 0: script embedded, injection hooked up in Phase 1.

_kumokara_preexec() {
    # OSC 133 ; C — command started (pre-exec)
    printf '\033]133;C\007'
}

_kumokara_precmd() {
    # OSC 133 ; D ; <exit_code> — command finished
    local exit_code=$?
    printf '\033]133;D;%s\007' "$exit_code"
    # OSC 7 — report current working directory
    printf '\033]7;file://%s%s\007' "$HOST" "$PWD"
}

_kumokara_prompt_marker() {
    # OSC 133 ; A — prompt marker (for internal use)
    printf '\033]133;A\007'
}

# Install hooks only if integration is enabled
if [[ -z "$KUMOKARA_DISABLE_INTEGRATION" ]]; then
    autoload -Uz add-zsh-hook
    add-zsh-hook preexec _kumokara_preexec
    add-zsh-hook precmd _kumokara_precmd

    # Add prompt marker to PROMPT
    PROMPT="%{$(_kumokara_prompt_marker)%}$PROMPT"
fi
