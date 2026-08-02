# fish shell integration for Kumokara
# Injects OSC 133 (FTCS) and OSC 7 sequences for terminal observability.
# Phase 0: script embedded, injection hooked up in Phase 1.

function _kumokara_preexec --on-event fish_preexec
    # OSC 133 ; C — command started (pre-exec)
    printf '\033]133;C\007'
end

function _kumokara_precmd --on-event fish_prompt
    # OSC 133 ; D ; 0 — fish doesn't easily track exit code per command
    printf '\033]133;D;0\007'
    # OSC 7 — report current working directory
    printf '\033]7;file://%s%s\007' (hostname) (pwd)
end

function _kumokara_prompt_marker --on-event fish_prompt
    # OSC 133 ; A — prompt marker (for internal use)
    printf '\033]133;A\007'
end
