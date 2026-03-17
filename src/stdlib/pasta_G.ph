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

# ---------------------------------------------------------------------
# Color helpers
# color values are r,g,b numbers in [0,255]
# ---------------------------------------------------------------------

# clamp a value to [0, 255]
set g_clamp = DO v:
    if v < 0:
        set v = 0
    if v > 255:
        set v = 255
    v
END

# pack r,g,b into a list for passing around
set g_color = DO r, g, b:
    [r, g, b]
END

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
# Window / canvas lifecycle
# ---------------------------------------------------------------------

# g_window(title, w, h) -> window_handle
set g_window = DO title, w, h:
    WINDOW(title, w, h)
END

# g_canvas(w, h) -> canvas_handle
set g_canvas = DO w, h:
    CANVAS(w, h)
END

# g_close(window)
set g_close = DO win:
    CLOSE(win)
END

# g_open(window) -> bool
set g_open = DO win:
    WINDOW_OPEN(win)
END

# g_show(window, canvas) — blit canvas to window
set g_show = DO win, canvas:
    BLIT(win, canvas)
END

# g_save(window, path) — save window framebuffer as PPM
set g_save = DO win, path:
    WINDOW_SAVE(win, path)
END

# ---------------------------------------------------------------------
# Pixel drawing
# ---------------------------------------------------------------------

# g_pixel(canvas, x, y, r, g, b)
set g_pixel = DO canvas, x, y, r, g, b:
    PIXEL(canvas, x, y, r, g, b)
END

# g_pixel_color(canvas, x, y, color) where color = [r,g,b]
set g_pixel_color = DO canvas, x, y, color:
    PIXEL(canvas, x, y, color[0], color[1], color[2])
END

# ---------------------------------------------------------------------
# Filled rectangle
# g_fill_rect(canvas, x, y, w, h, r, g, b)
# ---------------------------------------------------------------------
set g_fill_rect = DO canvas, rx, ry, rw, rh, r, g, b:
    set cy = ry
    while cy < (ry + rh):
        set cx = rx
        while cx < (rx + rw):
            PIXEL(canvas, cx, cy, r, g, b)
            set cx = cx + 1
        set cy = cy + 1
END

# g_fill_rect_color(canvas, x, y, w, h, color)
set g_fill_rect_color = DO canvas, rx, ry, rw, rh, color:
    g_fill_rect(canvas, rx, ry, rw, rh, color[0], color[1], color[2])
END

# ---------------------------------------------------------------------
# Clear canvas to a solid color
# g_clear(canvas, w, h, r, g, b)
# ---------------------------------------------------------------------
set g_clear = DO canvas, w, h, r, g, b:
    g_fill_rect(canvas, 0, 0, w, h, r, g, b)
END

# g_clear_color(canvas, w, h, color)
set g_clear_color = DO canvas, w, h, color:
    g_fill_rect(canvas, 0, 0, w, h, color[0], color[1], color[2])
END

# g_clear_black(canvas, w, h)
set g_clear_black = DO canvas, w, h:
    g_fill_rect(canvas, 0, 0, w, h, 0, 0, 0)
END

# ---------------------------------------------------------------------
# Line drawing (Bresenham)
# g_line(canvas, x0, y0, x1, y1, r, g, b)
# ---------------------------------------------------------------------
set g_line = DO canvas, x0, y0, x1, y1, r, g, b:
    set dx = x1 - x0
    set dy = y1 - y0
    if dx < 0:
        set dx = 0 - dx
    if dy < 0:
        set dy = 0 - dy
    set sx = 1
    if x0 > x1:
        set sx = 0 - 1
    set sy = 1
    if y0 > y1:
        set sy = 0 - 1
    set err = dx - dy
    set lx = x0
    set ly = y0
    set running = True
    while running:
        PIXEL(canvas, lx, ly, r, g, b)
        if lx == x1:
            if ly == y1:
                set running = False
        set e2 = err + err
        if e2 > (0 - dy):
            set err = err - dy
            set lx = lx + sx
        if e2 < dx:
            set err = err + dx
            set ly = ly + sy
END

# g_line_color(canvas, x0,y0, x1,y1, color)
set g_line_color = DO canvas, x0, y0, x1, y1, color:
    g_line(canvas, x0, y0, x1, y1, color[0], color[1], color[2])
END

# ---------------------------------------------------------------------
# Rectangle outline
# g_rect(canvas, x, y, w, h, r, g, b)
# ---------------------------------------------------------------------
set g_rect = DO canvas, rx, ry, rw, rh, r, g, b:
    g_line(canvas, rx,       ry,       rx+rw-1, ry,       r, g, b)
    g_line(canvas, rx,       ry+rh-1, rx+rw-1, ry+rh-1, r, g, b)
    g_line(canvas, rx,       ry,       rx,       ry+rh-1, r, g, b)
    g_line(canvas, rx+rw-1, ry,       rx+rw-1, ry+rh-1, r, g, b)
END

# ---------------------------------------------------------------------
# Circle (midpoint algorithm)
# g_circle(canvas, cx, cy, radius, r, g, b)
# ---------------------------------------------------------------------
set g_circle = DO canvas, cx, cy, radius, r, g, b:
    set px = radius
    set py = 0
    set err = 0
    while px >= py:
        PIXEL(canvas, cx+px, cy+py, r, g, b)
        PIXEL(canvas, cx+py, cy+px, r, g, b)
        PIXEL(canvas, cx-py, cy+px, r, g, b)
        PIXEL(canvas, cx-px, cy+py, r, g, b)
        PIXEL(canvas, cx-px, cy-py, r, g, b)
        PIXEL(canvas, cx-py, cy-px, r, g, b)
        PIXEL(canvas, cx+py, cy-px, r, g, b)
        PIXEL(canvas, cx+px, cy-py, r, g, b)
        set py = py + 1
        if err <= 0:
            set err = err + (py + py + 1)
        if err > 0:
            set px = px - 1
            set err = err + (1 - px - px)
END

# g_circle_color(canvas, cx, cy, radius, color)
set g_circle_color = DO canvas, cx, cy, radius, color:
    g_circle(canvas, cx, cy, radius, color[0], color[1], color[2])
END

# ---------------------------------------------------------------------
# Gradient fill helpers
# g_gradient_h(canvas, x, y, w, h, r0,g0,b0, r1,g1,b1)
# horizontal gradient from color0 (left) to color1 (right)
# ---------------------------------------------------------------------
set g_gradient_h = DO canvas, x, y, w, h, r0, g0, b0, r1, g1, b1:
    set cy = y
    while cy < (y + h):
        set cx = x
        while cx < (x + w):
            set t = (cx - x)
            set r = r0 + ((r1 - r0) * t / w)
            set g = g0 + ((g1 - g0) * t / w)
            set b = b0 + ((b1 - b0) * t / w)
            PIXEL(canvas, cx, cy, r, g, b)
            set cx = cx + 1
        set cy = cy + 1
END

# ---------------------------------------------------------------------
# Vertical gradient
# g_gradient_v(canvas, x, y, w, h, r0,g0,b0, r1,g1,b1)
# ---------------------------------------------------------------------
set g_gradient_v = DO canvas, x, y, w, h, r0, g0, b0, r1, g1, b1:
    set cy = y
    while cy < (y + h):
        set t = (cy - y)
        set r = r0 + ((r1 - r0) * t / h)
        set g = g0 + ((g1 - g0) * t / h)
        set b = b0 + ((b1 - b0) * t / h)
        set cx = x
        while cx < (x + w):
            PIXEL(canvas, cx, cy, r, g, b)
            set cx = cx + 1
        set cy = cy + 1
END

# ---------------------------------------------------------------------
# Event loop helper
# g_loop(window, canvas, frame_fn)
# Calls frame_fn(canvas) each frame until window is closed.
# frame_fn should draw into canvas; g_loop blits each frame.
# ---------------------------------------------------------------------
set g_loop = DO win, canvas, frame_fn:
    while WINDOW_OPEN(win):
        frame_fn(canvas)
        BLIT(win, canvas)
END

# ---------------------------------------------------------------------
# Simple run-once render + save
# g_render_save(title, w, h, draw_fn, path)
# Creates window+canvas, calls draw_fn(canvas), blits, saves PPM.
# ---------------------------------------------------------------------
set g_render_save = DO title, w, h, draw_fn, path:
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
