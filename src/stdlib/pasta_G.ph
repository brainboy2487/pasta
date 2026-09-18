# pasta_G.ph — Pasta Standard Graphics Library
# Version: 0.2
# Wired to real interpreter builtins:
#   WINDOW(title, w, h)         -> window_handle
#   CANVAS(w, h)                -> canvas_handle
#   PIXEL(canvas, x, y, r, g, b)
#   BLIT(window, canvas)
#   WINDOW_OPEN(window)         -> bool
#   WINDOW_SAVE(window, path)
#   CLOSE(window)
#
# All high-level helpers below are implemented in pure Pasta
# on top of those six primitives.

# common named colors as [r,g,b] lists
set G_BLACK   = [0,   0,   0  ]
set G_WHITE   = [255, 255, 255]
set G_RED     = [255, 0,   0  ]
set G_GREEN   = [0,   255, 0  ]
set G_BLUE    = [0,   0,   255]
set G_YELLOW  = [255, 255, 0  ]
set G_CYAN    = [0,   255, 255]
set G_MAGENTA = [255, 0,   255]
set G_GRAY    = [128, 128, 128]
set G_ORANGE  = [255, 128, 0  ]
# ---------------------------------------------------------------------
# Color helpers
# color values are r,g,b numbers in [0,255]
# ---------------------------------------------------------------------





# clamp a value to [0, 255]
DEF g_clamp(v):
    IF v < 0:
        set v = 0
    END
    IF v > 255:
        set v = 255
    END
    v
END

# NOTE: WINDOW_KEY is a native builtin - no wrapper needed
# Just call WINDOW_KEY(win) directly from user code

# Window / canvas lifecycle


# pack r,g,b into a list for passing around
DEF g_color(r, g, b):
    [r, g, b]
END

DEF g_window(title, w, h):
    WINDOW(title, w, h)
END

# g_canvas(w, h) -> canvas_handle
DEF g_canvas(w, h):
    CANVAS(w, h)
END

# g_close(window)
DEF g_close(win):
    CLOSE(win)
END

DEF g_open(win):
    RET.NOW(): WINDOW_OPEN(win)
END

# g_show(window, canvas) — blit canvas to window
DEF g_show(win, canvas):
    BLIT(win, canvas)
END

DEF g_save(win, path):
    WINDOW_SAVE(win, path)
END

DEF g_pixel(canvas, x, y, r, g, b):
    PIXEL(canvas, x, y, r, g, b)
END

# ---------------------------------------------------------------------
# Pixel drawing
DEF g_pixel_color(canvas, x, y, color):
    PIXEL(canvas, x, y, color[0], color[1], color[2])
END


# canvas_fill_rect is now a native builtin in the executor
# No wrapper needed - it's called directly


# Use the optimized native fill for g_fill_rect
DEF g_fill_rect(canvas, rx, ry, rw, rh, r, g, b):
    canvas_fill_rect(canvas, rx, ry, rw, rh, r, g, b)
END

# g_pixel_color(canvas, x, y, color) where color = [r,g,b]
# g_fill_rect_color(canvas, x, y, w, h, color)

DEF g_fill_rect_color(canvas, rx, ry, rw, rh, color):
    g_fill_rect(canvas, rx, ry, rw, rh, color[0], color[1], color[2])
END

# g_clear(canvas, w, h, r, g, b)
DEF g_clear(canvas, w, h, r, g, b):
    g_fill_rect(canvas, 0, 0, w, h, r, g, b)
END

# g_clear_color(canvas, w, h, color)
DEF g_clear_color(canvas, w, h, color):
    g_fill_rect(canvas, 0, 0, w, h, color[0], color[1], color[2])
END

DEF g_clear_black(canvas, w, h):
    g_fill_rect(canvas, 0, 0, w, h, 0, 0, 0)
END

# ---------------------------------------------------------------------
# Clear canvas to a solid color
# g_clear(canvas, w, h, r, g, b)
# ---------------------------------------------------------------------

# g_clear_color(canvas, w, h, color)

# g_clear_black(canvas, w, h)

# ---------------------------------------------------------------------

# Line drawing (Bresenham)
# g_line(canvas, x0, y0, x1, y1, r, g, b)
# ---------------------------------------------------------------------
DEF g_line(canvas, x0, y0, x1, y1, r, g, b):
    set dx = x1 - x0
    set dy = y1 - y0
    IF dx < 0:
        set dx = 0 - dx
    END
    IF dy < 0:
        set dy = 0 - dy
    END
    set sx = 1
    IF x0 > x1:
        set sx = 0 - 1
    END
    set sy = 1
    IF y0 > y1:
        set sy = 0 - 1
    END
    set err = dx - dy
    set lx = x0
    set ly = y0
    set running = True
    WHILE running:
        PIXEL(canvas, lx, ly, r, g, b)
        IF lx == x1:
            IF ly == y1:
                set running = False
            END
        END
        set e2 = err + err
        IF e2 > (0 - dy):
            set err = err - dy
            set lx = lx + sx
        END
        IF e2 < dx:
            set err = err + dx
            set ly = ly + sy
        END
    END
END

# g_line_color(canvas, x0,y0, x1,y1, color)
DEF g_line_color(canvas, x0, y0, x1, y1, color):
    g_line(canvas, x0, y0, x1, y1, color[0], color[1], color[2])
END

# ---------------------------------------------------------------------
# Rectangle outline
# ---------------------------------------------------------------------
DEF g_rect(canvas, rx, ry, rw, rh, r, g, b):
    g_line(canvas, rx,       ry,       rx+rw-1, ry,       r, g, b)
    g_line(canvas, rx,       ry+rh-1, rx+rw-1, ry+rh-1, r, g, b)
    g_line(canvas, rx,       ry,       rx,       ry+rh-1, r, g, b)
    g_line(canvas, rx+rw-1, ry,       rx+rw-1, ry+rh-1, r, g, b)
END

# ---------------------------------------------------------------------
# Circle (midpoint algorithm)
# g_circle(canvas, cx, cy, radius, r, g, b)
# ---------------------------------------------------------------------
DEF g_circle(canvas, cx, cy, radius, r, g, b):
    set px = radius
    set py = 0
    set err = 0
    WHILE px >= py:
        PIXEL(canvas, cx+py, cy+px, r, g, b)
        PIXEL(canvas, cx-py, cy+px, r, g, b)
        PIXEL(canvas, cx-px, cy+py, r, g, b)
        PIXEL(canvas, cx-px, cy-py, r, g, b)
        PIXEL(canvas, cx-py, cy-px, r, g, b)
        PIXEL(canvas, cx+py, cy-px, r, g, b)
        PIXEL(canvas, cx+px, cy-py, r, g, b)
        set py = py + 1
        IF err <= 0:
            set err = err + (py + py + 1)
        END
        IF err > 0:
            set px = px - 1
            set err = err + (1 - px - px)
        END
    END
END

# g_circle_color(canvas, cx, cy, radius, color)
DEF g_circle_color(canvas, cx, cy, radius, color):
    g_circle(canvas, cx, cy, radius, color[0], color[1], color[2])
END

# ---------------------------------------------------------------------
# Gradient fill helpers
# g_gradient_h(canvas, x, y, w, h, r0,g0,b0, r1,g1,b1)
# horizontal gradient from color0 (left) to color1 (right)
# ---------------------------------------------------------------------
DEF g_gradient_h(canvas, x, y, w, h, r0, g0, b0, r1, g1, b1):
    set cy = y
    WHILE cy < (y + h):
        set cx = x
        WHILE cx < (x + w):
            set t = (cx - x)
            set r = r0 + ((r1 - r0) * t / w)
            set g = g0 + ((g1 - g0) * t / w)
            set b = b0 + ((b1 - b0) * t / w)
            PIXEL(canvas, cx, cy, r, g, b)
            set cx = cx + 1
        END
        set cy = cy + 1
    END
END

# ---------------------------------------------------------------------
# Vertical gradient
# ---------------------------------------------------------------------
DEF g_gradient_v(canvas, x, y, w, h, r0, g0, b0, r1, g1, b1):
    set cy = y
    WHILE cy < (y + h):
        set t = (cy - y)
        set r = r0 + ((r1 - r0) * t / h)
        set g = g0 + ((g1 - g0) * t / h)
        set b = b0 + ((b1 - b0) * t / h)
        set cx = x
        WHILE cx < (x + w):
            PIXEL(canvas, cx, cy, r, g, b)
            set cx = cx + 1
        END
        set cy = cy + 1
    END
END

# ---------------------------------------------------------------------
# Event loop helper
# g_loop(window, canvas, frame_fn)
# Calls frame_fn(canvas) each frame until window is closed.
# frame_fn should draw into canvas; g_loop blits each frame.
# ---------------------------------------------------------------------
DEF g_loop(win, canvas, frame_fn):
    WHILE WINDOW_OPEN(win):
        frame_fn(canvas)
        BLIT(win, canvas)
    END
END

# ---------------------------------------------------------------------
# Advanced game loop helpers
# ---------------------------------------------------------------------
# g_game_loop(win, canvas, frame_fn, target_fps)
# Calls frame_fn(canvas, dt) each frame, limiting FPS, returns when window closes.
DEF g_game_loop(win, canvas, frame_fn, target_fps):
    set paused = False
    set last_ms = g_now()
    WHILE WINDOW_OPEN(win):
        set start_ms = g_now()
        set dt = start_ms - last_ms
        last_ms = start_ms
        IF NOT paused:
            frame_fn(canvas, dt)
        END
        BLIT(win, canvas)
        g_fps_limit(start_ms, target_fps)
    END
END

# ---------------------------------------------------------------------
# Framerate and timing helpers
# ---------------------------------------------------------------------
# g_now() -> current time in ms (float)
#   Returns the current time in milliseconds (float). Useful for frame timing.
DEF g_now():
    RET.NOW(): TIME_MS()
END

# g_sleep(ms)
#   Sleep for ms milliseconds. Use for frame limiting or delays.
DEF g_sleep(ms):
    SLEEP(ms)
END

# g_fps_limit(start_ms, target_fps)
#   Sleep to maintain target FPS. Call at end of frame, passing frame start time and target FPS.
DEF g_fps_limit(start_ms, target_fps):
    set frame_ms = 1000 / target_fps
    set elapsed = g_now() - start_ms
    IF elapsed < frame_ms:
        g_sleep(frame_ms - elapsed)
    END
END

# g_frame_delta(last_ms)
#   Returns ms since last_ms. Use to compute frame delta time.
DEF g_frame_delta(last_ms):
    g_now() - last_ms
END

# g_pause_loop(win, canvas, frame_fn, target_fps)
# Like g_game_loop, but supports pausing/resuming with a global 'paused' variable
DEF g_pause_loop(win, canvas, frame_fn, target_fps):
    set paused = False
    set last_ms = g_now()
    WHILE WINDOW_OPEN(win):
        set start_ms = g_now()
        set dt = start_ms - last_ms
        last_ms = start_ms
        IF NOT paused:
            frame_fn(canvas, dt)
        END
        BLIT(win, canvas)
        g_fps_limit(start_ms, target_fps)
    END
END


# ---------------------------------------------------------------------
# Input and event helpers (keyboard)
# ---------------------------------------------------------------------
# g_key(win) — returns last key pressed (or None)
DEF g_key(win):
    RET.NOW(): WINDOW_KEY(win)
END

# g_key_poll(win, key) — returns True if key is currently pressed
DEF g_key_poll(win, key):
    RET.NOW(): WINDOW_KEY(win) == key
END

# g_wait_key(win) — blocks until a key is pressed, returns key
DEF g_wait_key(win):
    set k = None
    WHILE k == None:
        set k = WINDOW_KEY(win)
        g_sleep(10)
    END
    k
END

# ---------------------------------------------------------------------
# Input and event helpers (mouse, window) — API stub, backend TODO
# ---------------------------------------------------------------------
# g_mouse_pos(win) — returns [x, y] or None if not available
#   TODO: Implement in backend (X11: track MotionNotify, ButtonPress)
DEF g_mouse_pos(win):
    # TODO: Native backend must provide mouse position
    None
END

# g_mouse_button(win) — returns last mouse button event as [button, state, x, y] or None
#   button: 1=left, 2=middle, 3=right; state: "down" or "up"
#   TODO: Implement in backend (X11: track ButtonPress/ButtonRelease)
DEF g_mouse_button(win):
    # TODO: Native backend must provide mouse button events
    None
END

# g_window_event(win) — returns last window event as string ("close", "resize", etc) or None
#   TODO: Implement in backend (X11: StructureNotifyMask, etc)
DEF g_window_event(win):
    # TODO: Native backend must provide window events
    None
END

# ---------------------------------------------------------------------
# Simple run-once render + save
# g_render_save(title, w, h, draw_fn, path)
# Creates window+canvas, calls draw_fn(canvas), blits, saves PPM.
# ---------------------------------------------------------------------
DEF g_render_save(title, w, h, draw_fn, path):
    set win    = WINDOW(title, w, h)
    set canvas = CANVAS(w, h)
    draw_fn(canvas)
    BLIT(win, canvas)
    WINDOW_SAVE(win, path)
    CLOSE(win)
END

# ---------------------------------------------------------------------
# End of pasta_G.ph
# ---------------------------------------------------------------------
