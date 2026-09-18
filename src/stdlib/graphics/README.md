Graphics stdlib template
=======================

Files:
- mod.rs: registration hooks for builtins.
- canvas.rs: CPU pixel buffer and SharedCanvas type.
- draw.rs: CPU drawing primitives (line, rect, circle).
- window.rs: platform-agnostic Window wrapper.
- backend/x11.rs: X11 backend stub (Linux).
- backend/win32.rs: Win32 backend stub (Windows).

Extension points:
- Implement X11/Win32 blit logic in backend/*.
- Register builtins in mod.rs to expose functions to Pasta.
- Use SharedCanvas (Arc<Mutex<Canvas>>) to pass canvas objects safely into the interpreter.
