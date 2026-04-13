# pasta_game.ph — PASTA Game Development Module
# Version: 1.0
# A comprehensive 2D game engine with input management, shapes, vectors,
# menus, FPS control, and performance optimizations.
#
# USAGE:
#   FROM pasta_game USE:
#       Game, Vec2, Rect, Circle, Sprite, Menu, Input
#   END
#
# Or use individual functions directly after import.

set __header_pasta_game = "pasta_game v1.0 loaded"

# =====================================================================
# SECTION 1: VECTOR MATH (Vec2)
# =====================================================================
# 2D vectors represented as [x, y] lists

# Create a new 2D vector
DEF vec2(x, y):
    [x, y]
END

# Vector zero constant
set VEC2_ZERO = [0, 0]
set VEC2_ONE = [1, 1]
set VEC2_UP = [0, -1]
set VEC2_DOWN = [0, 1]
set VEC2_LEFT = [-1, 0]
set VEC2_RIGHT = [1, 0]

# Vector addition: a + b
DEF vec2_add(a, b):
    [a[0] + b[0], a[1] + b[1]]
END

# Vector subtraction: a - b
DEF vec2_sub(a, b):
    [a[0] - b[0], a[1] - b[1]]
END

# Scalar multiply: v * s
DEF vec2_scale(v, s):
    [v[0] * s, v[1] * s]
END

# Dot product
DEF vec2_dot(a, b):
    (a[0] * b[0]) + (a[1] * b[1])
END

# Magnitude squared (avoid sqrt for performance)
DEF vec2_len_sq(v):
    (v[0] * v[0]) + (v[1] * v[1])
END

# Distance squared between two points
DEF vec2_dist_sq(a, b):
    set dx = b[0] - a[0]
    set dy = b[1] - a[1]
    (dx * dx) + (dy * dy)
END

# Normalize (approximate for integer math)
# Returns unit-ish vector in same direction
DEF vec2_normalize(v):
    set len_sq = vec2_len_sq(v)
    IF len_sq == 0:
        RET.NOW(): [0, 0]
    END
    # Approximate normalization using integer scaling
    # Scale up, then divide by magnitude approximation
    set scale = 100
    set mag = 1
    # Simple magnitude approximation: max(|x|,|y|) + min(|x|,|y|)/2
    set ax = v[0]
    set ay = v[1]
    IF ax < 0:
        set ax = 0 - ax
    END
    IF ay < 0:
        set ay = 0 - ay
    END
    IF ax > ay:
        set mag = ax + (ay / 2)
    ELSE:
        set mag = ay + (ax / 2)
    END
    IF mag == 0:
        set mag = 1
    END
    [(v[0] * scale) / mag, (v[1] * scale) / mag]
END

# Lerp between two vectors: a + t*(b-a) where t is 0-100 (percentage)
DEF vec2_lerp(a, b, t):
    set dx = b[0] - a[0]
    set dy = b[1] - a[1]
    [a[0] + (dx * t / 100), a[1] + (dy * t / 100)]
END

# =====================================================================
# SECTION 2: SHAPES & GEOMETRY
# =====================================================================

# Rectangle: [x, y, width, height]
DEF rect(x, y, w, h):
    [x, y, w, h]
END

# Get rect properties
DEF rect_x(r):
    r[0]
END
DEF rect_y(r):
    r[1]
END
DEF rect_w(r):
    r[2]
END
DEF rect_h(r):
    r[3]
END
DEF rect_right(r):
    r[0] + r[2]
END
DEF rect_bottom(r):
    r[1] + r[3]
END
DEF rect_center(r):
    [r[0] + (r[2] / 2), r[1] + (r[3] / 2)]
END

# Move rect by delta
DEF rect_move(r, dx, dy):
    [r[0] + dx, r[1] + dy, r[2], r[3]]
END

# Check if point is inside rect
DEF rect_contains(r, px, py):
    set inside = True
    IF px < r[0]:
        set inside = False
    END
    IF px >= (r[0] + r[2]):
        set inside = False
    END
    IF py < r[1]:
        set inside = False
    END
    IF py >= (r[1] + r[3]):
        set inside = False
    END
    inside
END

# Check if two rects overlap (AABB collision)
DEF rect_intersects(a, b):
    set no_overlap = False
    IF a[0] >= (b[0] + b[2]):
        set no_overlap = True
    END
    IF (a[0] + a[2]) <= b[0]:
        set no_overlap = True
    END
    IF a[1] >= (b[1] + b[3]):
        set no_overlap = True
    END
    IF (a[1] + a[3]) <= b[1]:
        set no_overlap = True
    END
    NOT no_overlap
END

# Circle: [cx, cy, radius]
DEF circle(cx, cy, r):
    [cx, cy, r]
END

# Check if point is inside circle (uses squared distance)
DEF circle_contains(c, px, py):
    set dx = px - c[0]
    set dy = py - c[1]
    set dist_sq = (dx * dx) + (dy * dy)
    set r_sq = c[2] * c[2]
    dist_sq <= r_sq
END

# Check if two circles overlap
DEF circle_intersects(a, b):
    set dx = b[0] - a[0]
    set dy = b[1] - a[1]
    set dist_sq = (dx * dx) + (dy * dy)
    set r_sum = a[2] + b[2]
    set r_sum_sq = r_sum * r_sum
    dist_sq <= r_sum_sq
END

# Check if circle and rect overlap
DEF circle_rect_intersects(circ, r):
    # Find closest point on rect to circle center
    set cx = circ[0]
    set cy = circ[1]
    set closest_x = cx
    set closest_y = cy
    
    IF cx < r[0]:
        set closest_x = r[0]
    END
    IF cx > (r[0] + r[2]):
        set closest_x = r[0] + r[2]
    END
    IF cy < r[1]:
        set closest_y = r[1]
    END
    IF cy > (r[1] + r[3]):
        set closest_y = r[1] + r[3]
    END
    
    set dx = cx - closest_x
    set dy = cy - closest_y
    set dist_sq = (dx * dx) + (dy * dy)
    set r_sq = circ[2] * circ[2]
    dist_sq <= r_sq
END

# =====================================================================
# SECTION 3: DRAWING PRIMITIVES (Optimized)
# =====================================================================

# Draw filled rectangle (uses native fast fill)
DEF draw_rect(canvas, r, color):
    canvas_fill_rect(canvas, r[0], r[1], r[2], r[3], color[0], color[1], color[2])
END

# Draw rectangle outline
DEF draw_rect_outline(canvas, r, color):
    set x = r[0]
    set y = r[1]
    set w = r[2]
    set h = r[3]
    # Top
    canvas_fill_rect(canvas, x, y, w, 1, color[0], color[1], color[2])
    # Bottom
    canvas_fill_rect(canvas, x, y + h - 1, w, 1, color[0], color[1], color[2])
    # Left
    canvas_fill_rect(canvas, x, y, 1, h, color[0], color[1], color[2])
    # Right
    canvas_fill_rect(canvas, x + w - 1, y, 1, h, color[0], color[1], color[2])
END

# Draw filled circle (midpoint algorithm with horizontal spans)
DEF draw_circle(canvas, c, color):
    set cx = c[0]
    set cy = c[1]
    set radius = c[2]
    set px = radius
    set py = 0
    set err = 0
    
    WHILE px >= py:
        # Draw horizontal spans for filled circle
        canvas_fill_rect(canvas, cx - px, cy + py, px + px + 1, 1, color[0], color[1], color[2])
        canvas_fill_rect(canvas, cx - px, cy - py, px + px + 1, 1, color[0], color[1], color[2])
        canvas_fill_rect(canvas, cx - py, cy + px, py + py + 1, 1, color[0], color[1], color[2])
        canvas_fill_rect(canvas, cx - py, cy - px, py + py + 1, 1, color[0], color[1], color[2])
        
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

# Draw circle outline
DEF draw_circle_outline(canvas, c, color):
    set cx = c[0]
    set cy = c[1]
    set radius = c[2]
    set px = radius
    set py = 0
    set err = 0
    
    WHILE px >= py:
        PIXEL(canvas, cx + px, cy + py, color[0], color[1], color[2])
        PIXEL(canvas, cx - px, cy + py, color[0], color[1], color[2])
        PIXEL(canvas, cx + px, cy - py, color[0], color[1], color[2])
        PIXEL(canvas, cx - px, cy - py, color[0], color[1], color[2])
        PIXEL(canvas, cx + py, cy + px, color[0], color[1], color[2])
        PIXEL(canvas, cx - py, cy + px, color[0], color[1], color[2])
        PIXEL(canvas, cx + py, cy - px, color[0], color[1], color[2])
        PIXEL(canvas, cx - py, cy - px, color[0], color[1], color[2])
        
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

# Draw line (Bresenham)
DEF draw_line(canvas, x0, y0, x1, y1, color):
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
        PIXEL(canvas, lx, ly, color[0], color[1], color[2])
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

# Draw triangle outline
DEF draw_triangle(canvas, x0, y0, x1, y1, x2, y2, color):
    draw_line(canvas, x0, y0, x1, y1, color)
    draw_line(canvas, x1, y1, x2, y2, color)
    draw_line(canvas, x2, y2, x0, y0, color)
END

# Draw polygon outline from list of [x,y] points
DEF draw_polygon(canvas, points, color):
    set n = len(points)
    IF n < 2:
        RET.NOW(): None
    END
    FOR i IN range(n):
        set j = (i + 1) % n
        set p0 = points[i]
        set p1 = points[j]
        draw_line(canvas, p0[0], p0[1], p1[0], p1[1], color)
    END
END

# =====================================================================
# SECTION 4: INPUT MANAGEMENT
# =====================================================================

# Key constants (common keys)
set KEY_NONE = 0
set KEY_UP = 65362
set KEY_DOWN = 65364
set KEY_LEFT = 65361
set KEY_RIGHT = 65363
set KEY_SPACE = 32
set KEY_ENTER = 65293
set KEY_ESCAPE = 65307
set KEY_W = 119
set KEY_A = 97
set KEY_S = 115
set KEY_D = 100
set KEY_Q = 113
set KEY_E = 101
set KEY_P = 112
set KEY_R = 114

# Input state structure: [last_key, keys_pressed_this_frame, keys_held]
# For simplicity, we track just the last key and provide helpers

# Create input state
DEF input_create():
    [0, 0, 0]  # [last_key, prev_key, frame_count]
END

# Update input state (call once per frame)
DEF input_update(state, win):
    set key = WINDOW_KEY(win)
    set new_state = [key, state[0], state[2] + 1]
    new_state
END

# Check if key was just pressed this frame
DEF input_pressed(state, key):
    set is_pressed = False
    IF state[0] == key:
        IF state[1] != key:
            set is_pressed = True
        END
    END
    is_pressed
END

# Check if key is currently held
DEF input_held(state, key):
    state[0] == key
END

# Get current key
DEF input_key(state):
    state[0]
END

# Direction from WASD or arrow keys as [dx, dy]
DEF input_direction(state):
    set dx = 0
    set dy = 0
    set key = state[0]
    
    IF key == KEY_W:
        set dy = -1
    END
    IF key == KEY_UP:
        set dy = -1
    END
    IF key == KEY_S:
        set dy = 1
    END
    IF key == KEY_DOWN:
        set dy = 1
    END
    IF key == KEY_A:
        set dx = -1
    END
    IF key == KEY_LEFT:
        set dx = -1
    END
    IF key == KEY_D:
        set dx = 1
    END
    IF key == KEY_RIGHT:
        set dx = 1
    END
    
    [dx, dy]
END

# =====================================================================
# SECTION 5: FPS & TIMING SYSTEM
# =====================================================================

# FPS counter state: [frame_count, last_second_time, current_fps]
DEF fps_create():
    set now = TIME_MS()
    [0, now, 0]
END

# Update FPS counter, returns current FPS
DEF fps_update(state):
    set now = TIME_MS()
    set frames = state[0] + 1
    set elapsed = now - state[1]
    set fps = state[2]
    
    IF elapsed >= 1000:
        set fps = frames
        set frames = 0
        set state = [frames, now, fps]
    ELSE:
        set state = [frames, state[1], fps]
    END
    
    state
END

# Get current FPS value
DEF fps_get(state):
    state[2]
END

# Frame rate limiter
DEF fps_limit(target_fps, frame_start):
    set target_ms = 1000 / target_fps
    set elapsed = TIME_MS() - frame_start
    IF elapsed < target_ms:
        set sleep_time = target_ms - elapsed
        SLEEP(sleep_time)
    END
END

# Delta time calculator: returns [new_last_time, delta_ms]
DEF delta_time(last_time):
    set now = TIME_MS()
    set delta = now - last_time
    [now, delta]
END

# =====================================================================
# SECTION 6: SPRITE & ENTITY SYSTEM
# =====================================================================

# Sprite: [x, y, width, height, color, visible, velocity_x, velocity_y]
DEF sprite_create(x, y, w, h, color):
    [x, y, w, h, color, True, 0, 0]
END

# Sprite getters
DEF sprite_x(s):
    s[0]
END
DEF sprite_y(s):
    s[1]
END
DEF sprite_w(s):
    s[2]
END
DEF sprite_h(s):
    s[3]
END
DEF sprite_color(s):
    s[4]
END
DEF sprite_visible(s):
    s[5]
END
DEF sprite_vx(s):
    s[6]
END
DEF sprite_vy(s):
    s[7]
END

# Get sprite as rect for collision
DEF sprite_rect(s):
    [s[0], s[1], s[2], s[3]]
END

# Move sprite by velocity * dt (dt in frames or fixed units)
DEF sprite_move(s, dt):
    set new_x = s[0] + (s[6] * dt)
    set new_y = s[1] + (s[7] * dt)
    [new_x, new_y, s[2], s[3], s[4], s[5], s[6], s[7]]
END

# Set sprite position
DEF sprite_set_pos(s, x, y):
    [x, y, s[2], s[3], s[4], s[5], s[6], s[7]]
END

# Set sprite velocity
DEF sprite_set_vel(s, vx, vy):
    [s[0], s[1], s[2], s[3], s[4], s[5], vx, vy]
END

# Set sprite visibility
DEF sprite_set_visible(s, v):
    [s[0], s[1], s[2], s[3], s[4], v, s[6], s[7]]
END

# Draw sprite
DEF sprite_draw(canvas, s):
    IF s[5]:
        set r = [s[0], s[1], s[2], s[3]]
        draw_rect(canvas, r, s[4])
    END
END

# Check if two sprites collide
DEF sprite_collides(a, b):
    IF NOT a[5]:
        RET.NOW(): False
    END
    IF NOT b[5]:
        RET.NOW(): False
    END
    rect_intersects(sprite_rect(a), sprite_rect(b))
END

# =====================================================================
# SECTION 7: MENU SYSTEM
# =====================================================================

# Menu item: [text, x, y, width, height, selected, enabled, action_id]
DEF menu_item(text, x, y, w, h, action_id):
    [text, x, y, w, h, False, True, action_id]
END

# Menu: [items_list, selected_index, bg_color, text_color, select_color]
DEF menu_create(bg_color, text_color, select_color):
    [[], 0, bg_color, text_color, select_color]
END

# Add item to menu
DEF menu_add_item(menu, item):
    set items = menu[0]
    # Note: list append would be items + [item] but we return new menu
    [[item], menu[1], menu[2], menu[3], menu[4]]
END

# Menu navigation - move selection up
DEF menu_up(menu):
    set idx = menu[1]
    set n = len(menu[0])
    IF n > 0:
        set idx = (idx - 1 + n) % n
    END
    [menu[0], idx, menu[2], menu[3], menu[4]]
END

# Menu navigation - move selection down
DEF menu_down(menu):
    set idx = menu[1]
    set n = len(menu[0])
    IF n > 0:
        set idx = (idx + 1) % n
    END
    [menu[0], idx, menu[2], menu[3], menu[4]]
END

# Get selected item's action_id
DEF menu_select(menu):
    set idx = menu[1]
    set items = menu[0]
    IF len(items) > idx:
        set item = items[idx]
        RET.NOW(): item[7]
    END
    0
END

# Draw menu (simplified - draws rectangles for items)
DEF menu_draw(canvas, menu, x, y, item_height, item_spacing):
    set items = menu[0]
    set selected = menu[1]
    set bg = menu[2]
    set fg = menu[3]
    set sel = menu[4]
    
    set cy = y
    FOR i IN range(len(items)):
        set item = items[i]
        set color = bg
        IF i == selected:
            set color = sel
        END
        # Draw item background
        canvas_fill_rect(canvas, x, cy, item[3], item_height, color[0], color[1], color[2])
        # Draw border
        set r = [x, cy, item[3], item_height]
        draw_rect_outline(canvas, r, fg)
        set cy = cy + item_height + item_spacing
    END
END

# =====================================================================
# SECTION 8: GAME STATE MANAGEMENT
# =====================================================================

# Game state enum-like constants
set STATE_MENU = 0
set STATE_PLAYING = 1
set STATE_PAUSED = 2
set STATE_GAMEOVER = 3

# Game context: [state, score, lives, level, custom_data]
DEF game_create():
    [STATE_MENU, 0, 3, 1, None]
END

DEF game_state(g):
    g[0]
END
DEF game_score(g):
    g[1]
END
DEF game_lives(g):
    g[2]
END
DEF game_level(g):
    g[3]
END
DEF game_data(g):
    g[4]
END

DEF game_set_state(g, state):
    [state, g[1], g[2], g[3], g[4]]
END
DEF game_add_score(g, points):
    [g[0], g[1] + points, g[2], g[3], g[4]]
END
DEF game_lose_life(g):
    [g[0], g[1], g[2] - 1, g[3], g[4]]
END
DEF game_next_level(g):
    [g[0], g[1], g[2], g[3] + 1, g[4]]
END
DEF game_set_data(g, data):
    [g[0], g[1], g[2], g[3], data]
END

# =====================================================================
# SECTION 9: CAMERA & VIEWPORT
# =====================================================================

# Camera: [x, y, width, height, zoom_level_percent]
DEF camera_create(w, h):
    [0, 0, w, h, 100]
END

DEF camera_move(cam, dx, dy):
    [cam[0] + dx, cam[1] + dy, cam[2], cam[3], cam[4]]
END

DEF camera_set_pos(cam, x, y):
    [x, y, cam[2], cam[3], cam[4]]
END

DEF camera_center_on(cam, target_x, target_y):
    set x = target_x - (cam[2] / 2)
    set y = target_y - (cam[3] / 2)
    [x, y, cam[2], cam[3], cam[4]]
END

# Transform world coords to screen coords
DEF camera_world_to_screen(cam, wx, wy):
    set sx = wx - cam[0]
    set sy = wy - cam[1]
    [sx, sy]
END

# Transform screen coords to world coords
DEF camera_screen_to_world(cam, sx, sy):
    set wx = sx + cam[0]
    set wy = sy + cam[1]
    [wx, wy]
END

# =====================================================================
# SECTION 10: PARTICLE SYSTEM (Simple)
# =====================================================================

# Particle: [x, y, vx, vy, life, max_life, color]
DEF particle_create(x, y, vx, vy, life, color):
    [x, y, vx, vy, life, life, color]
END

# Update particle, returns updated particle or None if dead
DEF particle_update(p):
    IF p[4] <= 0:
        RET.NOW(): None
    END
    set new_x = p[0] + p[2]
    set new_y = p[1] + p[3]
    set new_life = p[4] - 1
    [new_x, new_y, p[2], p[3], new_life, p[5], p[6]]
END

# Draw particle (size based on remaining life)
DEF particle_draw(canvas, p):
    IF p[4] > 0:
        set size = 1 + (p[4] * 3 / p[5])
        set r = [p[0] - (size/2), p[1] - (size/2), size, size]
        draw_rect(canvas, r, p[6])
    END
END

# =====================================================================
# SECTION 11: GRID UTILITIES (for tile-based games)
# =====================================================================

# Create grid as flat array with dimensions: returns [width, height, data_ptr]
# Uses PASTA's pointer system for efficient memory access
DEF grid_create(width, height, default_val):
    set size = width * height
    ALLOC.MEM(size) -> data
    
    # Initialize all cells
    FOR i IN range(size):
        GOTO data:
            SEEK data, i
            PUSH.BYTE default_val
        END
    END
    
    [width, height, data]
END

# Get grid cell value
DEF grid_get(grid, x, y):
    set w = grid[0]
    set h = grid[1]
    set data = grid[2]
    
    # Bounds check
    IF x < 0:
        RET.NOW(): 0
    END
    IF y < 0:
        RET.NOW(): 0
    END
    IF x >= w:
        RET.NOW(): 0
    END
    IF y >= h:
        RET.NOW(): 0
    END
    
    set idx = (y * w) + x
    set val = 0
    GOTO data:
        SEEK data, idx
        PULL.BYTE -> val
    END
    val
END

# Set grid cell value
DEF grid_set(grid, x, y, val):
    set w = grid[0]
    set h = grid[1]
    set data = grid[2]
    
    # Bounds check
    IF x < 0:
        RET.NOW(): None
    END
    IF y < 0:
        RET.NOW(): None
    END
    IF x >= w:
        RET.NOW(): None
    END
    IF y >= h:
        RET.NOW(): None
    END
    
    set idx = (y * w) + x
    GOTO data:
        SEEK data, idx
        PUSH.BYTE val
    END
END

# Draw grid with cell_size pixels per cell
DEF grid_draw(canvas, grid, cell_size, alive_color, dead_color):
    set w = grid[0]
    set h = grid[1]
    set data = grid[2]
    
    FOR y IN range(h):
        FOR x IN range(w):
            set idx = (y * w) + x
            set val = 0
            GOTO data:
                SEEK data, idx
                PULL.BYTE -> val
            END
            
            set screen_x = x * cell_size
            set screen_y = y * cell_size
            set color = dead_color
            IF val > 0:
                set color = alive_color
            END
            canvas_fill_rect(canvas, screen_x, screen_y, cell_size, cell_size, color[0], color[1], color[2])
        END
    END
END

# Free grid memory
DEF grid_free(grid):
    FREE grid[2]
END

# =====================================================================
# SECTION 12: COLOR UTILITIES
# =====================================================================

# Common colors
set COLOR_BLACK = [0, 0, 0]
set COLOR_WHITE = [255, 255, 255]
set COLOR_RED = [255, 0, 0]
set COLOR_GREEN = [0, 255, 0]
set COLOR_BLUE = [0, 0, 255]
set COLOR_YELLOW = [255, 255, 0]
set COLOR_CYAN = [0, 255, 255]
set COLOR_MAGENTA = [255, 0, 255]
set COLOR_ORANGE = [255, 128, 0]
set COLOR_PURPLE = [128, 0, 255]
set COLOR_GRAY = [128, 128, 128]
set COLOR_DARK_GRAY = [64, 64, 64]
set COLOR_LIGHT_GRAY = [192, 192, 192]

# Create color from RGB
DEF color_rgb(r, g, b):
    [r, g, b]
END

# Darken color by percentage (0-100)
DEF color_darken(c, percent):
    set factor = 100 - percent
    [(c[0] * factor) / 100, (c[1] * factor) / 100, (c[2] * factor) / 100]
END

# Lighten color by percentage (0-100)  
DEF color_lighten(c, percent):
    set r = c[0] + ((255 - c[0]) * percent / 100)
    set g = c[1] + ((255 - c[1]) * percent / 100)
    set b = c[2] + ((255 - c[2]) * percent / 100)
    [r, g, b]
END

# Lerp between two colors (t = 0-100)
DEF color_lerp(c0, c1, t):
    set r = c0[0] + ((c1[0] - c0[0]) * t / 100)
    set g = c0[1] + ((c1[1] - c0[1]) * t / 100)
    set b = c0[2] + ((c1[2] - c0[2]) * t / 100)
    [r, g, b]
END

# =====================================================================
# SECTION 13: MAIN GAME LOOP HELPER
# =====================================================================

# Standard game loop with FPS limiting and input handling
# Usage:
#   set game = game_loop_create(win, canvas, 60)
#   WHILE game_loop_running(game):
#       set game = game_loop_begin(game)
#       # Your update/draw code here using:
#       # - game_loop_input(game) for input state
#       # - game_loop_dt(game) for delta time
#       # - game_loop_fps(game) for current FPS
#       set game = game_loop_end(game)
#   END

# Game loop state: [win, canvas, target_fps, running, input_state, fps_state, last_time, dt]
DEF game_loop_create(win, canvas, target_fps):
    set input = input_create()
    set fps = fps_create()
    set now = TIME_MS()
    [win, canvas, target_fps, True, input, fps, now, 0]
END

DEF game_loop_running(loop):
    IF NOT loop[3]:
        RET.NOW(): False
    END
    WINDOW_OPEN(loop[0])
END

DEF game_loop_begin(loop):
    set frame_start = TIME_MS()
    set dt_result = delta_time(loop[6])
    set new_input = input_update(loop[4], loop[0])
    
    [loop[0], loop[1], loop[2], loop[3], new_input, loop[5], dt_result[0], dt_result[1]]
END

DEF game_loop_end(loop):
    BLIT(loop[0], loop[1])
    fps_limit(loop[2], loop[6])
    set new_fps = fps_update(loop[5])
    [loop[0], loop[1], loop[2], loop[3], loop[4], new_fps, loop[6], loop[7]]
END

DEF game_loop_input(loop):
    loop[4]
END

DEF game_loop_dt(loop):
    loop[7]
END

DEF game_loop_fps(loop):
    fps_get(loop[5])
END

DEF game_loop_canvas(loop):
    loop[1]
END

DEF game_loop_stop(loop):
    [loop[0], loop[1], loop[2], False, loop[4], loop[5], loop[6], loop[7]]
END

# =====================================================================
# End of pasta_game.ph
# =====================================================================
