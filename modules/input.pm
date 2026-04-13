# ════════════════════════════════════════════════════════════════════════════
# INPUT MANAGER MODULE - input.pm
# Provides reliable input handling for graphical applications
# ════════════════════════════════════════════════════════════════════════════
#
# Usage:
#   FROM input USE init_input, poll_input, get_key, has_key
#
# Functions:
#   init_input(win_handle)  - Initialize input manager with window
#   poll_input()            - Poll for new events (call every frame)
#   has_key()               - Returns 1 if key is available, 0 otherwise
#   get_key()               - Returns key string and clears it
#   
# ════════════════════════════════════════════════════════════════════════════

MOD input:
    export init_input, poll_input, has_key, get_key, get_last_key

# Internal state - use underscore prefix for "private"
_win_handle = ""
_current_key = ""
_key_ready = 0

DEF init_input(win):
    _win_handle = win
    _current_key = ""
    _key_ready = 0
    RET.NOW(): 1
END

DEF poll_input():
    # Poll window for events and capture any key
    IF _win_handle != "":
        k = WINDOW_KEY(_win_handle)
        IF k != "":
            _current_key = k
            _key_ready = 1
        END
    END
    RET.NOW(): _key_ready
END

DEF has_key():
    RET.NOW(): _key_ready
END

DEF get_key():
    # Return current key and clear it
    result = _current_key
    _current_key = ""
    _key_ready = 0
    RET.NOW(): result
END

DEF get_last_key():
    # Return current key without clearing
    RET.NOW(): _current_key
END

END
