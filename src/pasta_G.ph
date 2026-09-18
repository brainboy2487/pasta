# pasta_G.ph — Basic graphics header for PastaLang
# Version: 0.1
# Purpose: Provide a small, stable 2D graphics API for scripts.
#
# Design goals:
# - Minimal, easy-to-implement runtime contract (few builtins).
# - Familiar primitives: canvas, pixel, line, rect, circle, fill, text, save, show.
# - Graceful fallback to no-op or textual output when graphics backend is absent.
#
# Runtime primitives required (implement these in the interpreter as builtins):
# - __pasta_g_create_canvas(width: number, height: number) -> canvas_id (string or number)
# - __pasta_g_destroy_canvas(canvas_id)
# - __pasta_g_clear(canvas_id, color_string)
# - __pasta_g_set_pixel(canvas_id, x: number, y: number, color_string)
# - __pasta_g_get_pixel(canvas_id, x: number, y: number) -> color_string
# - __pasta_g_draw_line(canvas_id, x1, y1, x2, y2, color_string, width)
# - __pasta_g_draw_rect(canvas_id, x, y, w, h, color_string, fill_bool, stroke_width)
# - __pasta_g_draw_circle(canvas_id, cx, cy, r, color_string, fill_bool, stroke_width)
# - __pasta_g_draw_text(canvas_id, x, y, text_string, font_name, font_size, color_string)
# - __pasta_g_blit(canvas_id, src_canvas_id, dx, dy)
# - __pasta_g_save(canvas_id, filename_string) -> bool
# - __pasta_g_show(canvas_id)  # present to screen / window; may be no-op in headless mode
# - __pasta_g_poll_events() -> list of event objects (optional; for interactive apps)
#
# If the runtime does not implement these builtins, the header will fall back to
# printing textual descriptions of drawing commands so scripts remain debuggable.

# ---------------------------------------------------------------------
# Color helpers
# ---------------------------------------------------------------------

# color_rgb(r,g,b) -> "rgb(r,g,b)"
set color_rgb = DO r, g, b:
    PRINT str("rgb(")  # placeholder to show intent if builtin missing
    # runtime should provide a string builder or return formatted string
    # best practice: runtime exposes a helper builtin "g_color_rgb" returning a string
    TRY:
        CALL "g_color_rgb"(r, g, b)
    OTHERWISE:
        # fallback: simple string
        str("rgb(") + str(r) + "," + str(g) + "," + str(b) + ")"
END

# color_hex("#RRGGBB") -> "#RRGGBB" (identity, validates if runtime helper exists)
set color_hex = DO s:
    TRY:
        CALL "g_color_hex"(s)
    OTHERWISE:
        s
END

# Transparent color helper
set color_rgba = DO r, g, b, a:
    TRY:
        CALL "g_color_rgba"(r, g, b, a)
    OTHERWISE:
        str("rgba(") + str(r) + "," + str(g) + "," + str(b) + "," + str(a) + ")"
END

# ---------------------------------------------------------------------
# Canvas lifecycle
# ---------------------------------------------------------------------

# create_canvas(width, height) -> canvas_id
set create_canvas = DO w, h:
    TRY:
        CALL "__pasta_g_create_canvas"(w, h)
    OTHERWISE:
        # fallback: return None to indicate no canvas available
        None
END

# destroy_canvas(canvas_id)
set destroy_canvas = DO cid:
    TRY:
        CALL "__pasta_g_destroy_canvas"(cid)
    OTHERWISE:
        # no-op
        set _ = 0
END

# clear(canvas_id, color)
set clear = DO cid, color:
    TRY:
        CALL "__pasta_g_clear"(cid, color)
    OTHERWISE:
        PRINT "G: clear"  # debug fallback
END

# save(canvas_id, filename) -> bool
set save = DO cid, filename:
    TRY:
        CALL "__pasta_g_save"(cid, filename)
    OTHERWISE:
        False
END

# show(canvas_id) -> present to screen (may open a window)
set show = DO cid:
    TRY:
        CALL "__pasta_g_show"(cid)
    OTHERWISE:
        PRINT "G: show (no-op in this runtime)"
END

# ---------------------------------------------------------------------
# Primitive drawing operations
# ---------------------------------------------------------------------

# set_pixel(canvas, x, y, color)
set set_pixel = DO cid, x, y, color:
    TRY:
        CALL "__pasta_g_set_pixel"(cid, x, y, color)
    OTHERWISE:
        PRINT "G: set_pixel " + str(x) + "," + str(y) + " " + str(color)
END

# get_pixel(canvas, x, y) -> color_string or None
set get_pixel = DO cid, x, y:
    TRY:
        CALL "__pasta_g_get_pixel"(cid, x, y)
    OTHERWISE:
        None
END

# draw_line(canvas, x1,y1, x2,y2, color, width)
set draw_line = DO cid, x1, y1, x2, y2, color, width:
    TRY:
        CALL "__pasta_g_draw_line"(cid, x1, y1, x2, y2, color, width)
    OTHERWISE:
        PRINT "G: line " + str(x1) + "," + str(y1) + " -> " + str(x2) + "," + str(y2)
END

# draw_rect(canvas, x,y, w,h, color, fill=false, stroke_width=1)
set draw_rect = DO cid, x, y, w, h, color, fill, stroke:
    # normalize optional args
    if fill == None:
        set fill = False
    if stroke == None:
        set stroke = 1
    TRY:
        CALL "__pasta_g_draw_rect"(cid, x, y, w, h, color, fill, stroke)
    OTHERWISE:
        PRINT "G: rect " + str(x) + "," + str(y) + " " + str(w) + "x" + str(h)
END

# draw_circle(canvas, cx,cy, r, color, fill=false, stroke_width=1)
set draw_circle = DO cid, cx, cy, r, color, fill, stroke:
    if fill == None:
        set fill = False
    if stroke == None:
        set stroke = 1
    TRY:
        CALL "__pasta_g_draw_circle"(cid, cx, cy, r, color, fill, stroke)
    OTHERWISE:
        PRINT "G: circle " + str(cx) + "," + str(cy) + " r=" + str(r)
END

# draw_text(canvas, x,y, text, font="sans", size=12, color)
set draw_text = DO cid, x, y, text, font, size, color:
    if font == None:
        set font = "sans"
    if size == None:
        set size = 12
    TRY:
        CALL "__pasta_g_draw_text"(cid, x, y, text, font, size, color)
    OTHERWISE:
        PRINT "G: text at " + str(x) + "," + str(y) + ": " + str(text)
END

# blit(src_canvas, dst_canvas, dx, dy)
set blit = DO dst, src, dx, dy:
    TRY:
        CALL "__pasta_g_blit"(dst, src, dx, dy)
    OTHERWISE:
        PRINT "G: blit"
END

# ---------------------------------------------------------------------
# Convenience higher-level helpers (implemented in terms of primitives)
# ---------------------------------------------------------------------

# fill_rect(canvas, x,y,w,h, color) -> alias for draw_rect with fill=true
set fill_rect = DO cid, x, y, w, h, color:
    CALL draw_rect(cid, x, y, w, h, color, True, 0)
END

# stroke_rect(canvas, x,y,w,h, color, stroke_width)
set stroke_rect = DO cid, x, y, w, h, color, stroke:
    CALL draw_rect(cid, x, y, w, h, color, False, stroke)
END

# fill_circle(canvas, cx,cy,r,color)
set fill_circle = DO cid, cx, cy, r, color:
    CALL draw_circle(cid, cx, cy, r, color, True, 0)
END

# clear_to_color(canvas, color)
set clear_to_color = DO cid, color:
    CALL clear(cid, color)
END

# ---------------------------------------------------------------------
# Simple animation loop helper (cooperative)
# - frame_fn is a lambda that receives (canvas_id, t, dt) and draws a frame
# - fps is target frames per second (optional)
# ---------------------------------------------------------------------
set animate = DO canvas, frame_fn, fps:
    if fps == None:
        set fps = 30
    # naive loop: runtime should provide sleep_ms builtin for timing
    set last_t = 0
    set running = True
    while running:
        # runtime should provide a monotonic time in seconds via "time_now"
        TRY:
            let now = CALL "time_now"()
        OTHERWISE:
            let now = 0
        let dt = now - last_t
        # call user frame function
        TRY:
            CALL frame_fn(canvas, now, dt)
        OTHERWISE:
            # if frame_fn fails, stop animation
            set running = False
        # present frame
        CALL show(canvas)
        # sleep to cap fps if runtime supports sleep_ms
        TRY:
            CALL "sleep_ms"( (1000 / fps) )
        OTHERWISE:
            # best-effort no-op
            set _ = 0
        set last_t = now
    END
END

# ---------------------------------------------------------------------
# Event polling (optional)
# - poll_events() returns a list of event objects if runtime supports it.
# - Event object fields: type ("mouse", "key", "quit"), x,y, key, modifiers
# ---------------------------------------------------------------------
set poll_events = DO:
    TRY:
        CALL "__pasta_g_poll_events"()
    OTHERWISE:
        []
END

# ---------------------------------------------------------------------
# Example usage
# ---------------------------------------------------------------------
# IMPORT "pasta_G.ph"
#
# let c = create_canvas(320, 240)
# if c != None:
#     clear(c, color_hex("#202020"))
#     draw_line(c, 10, 10, 300, 200, color_rgb(255,0,0), 2)
#     fill_rect(c, 50, 50, 100, 60, color_hex("#00FF00"))
#     draw_text(c, 20, 220, "Hello Pasta G", "sans", 14, color_hex("#FFFFFF"))
#     save(c, "out.png")
#     show(c)
#
# # Simple animation
# set frame = DO cid, t, dt:
#     clear(cid, color_hex("#000000"))
#     let x = (t * 50) % 320
#     fill_circle(cid, x, 120, 20, color_rgb(0, 128, 255))
# END
#
# animate(c, frame, 30)
#
# ---------------------------------------------------------------------
# Notes for runtime implementers
# ---------------------------------------------------------------------
# - The runtime may implement the primitives using any backend:
#   - SDL2 / sdl2_ttf for interactive windows
#   - Cairo / Skia for vector-backed drawing and PNG export
#   - headless SVG/PNG renderer for servers
# - Builtins should accept and return simple Pasta values (numbers, strings).
# - For portability, __pasta_g_save should support PNG and SVG filenames (by extension).
# - __pasta_g_show may open a window or be a no-op in headless environments.
# - Event objects returned by __pasta_g_poll_events should be simple maps or lists
#   that the interpreter can expose as Pasta values (e.g., list of { "type":"mouse", "x":.. }).
#
# ---------------------------------------------------------------------
# End of pasta_G.ph
