# PASTA Graphics Module - New Importable Graphics System
# This replaces the built-in graphics functions with a cleaner importable API

# Import this module to enable graphics functionality:
# INCLUDE graphics

# Initialize graphics extension
LOAD_EXTENSION("graphics")

PRINT "Graphics module loaded successfully!"
PRINT "Available functions: WINDOW_CREATE, WINDOW_IS_OPEN, WINDOW_CLOSE, WINDOW_POLL, WINDOW_KEY"
PRINT "Canvas functions: CANVAS_SET_PIXEL, CANVAS_PRESENT, CANVAS_CLEAR"
PRINT "Cleanup function: GRAPHICS_CLEANUP"