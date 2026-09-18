# tools/patches/patch1_window.py
# Insert a minimal headless `window` builtin into executor.rs
OPS = [
    {
        "type": "insert_after",
        "anchor": '"fs.mkdir" => {',
        "content": (
            '\n            // Simple headless window/canvas creation for tests and headless rendering.\n'
            '            // Usage: window(title:string, width:number, height:number) -> string handle\n'
            '            "window" => {\n'
            '                if args.len() != 3 { return Err(anyhow!("window expects 3 args (title, width, height)")); }\n'
            '                match (&args[0], &args[1], &args[2]) {\n'
            '                    (Value::String(_title), Value::Number(w), Value::Number(h)) => {\n'
            '                        let width = *w as usize;\n'
            '                        let height = *h as usize;\n'
            '                        if width == 0 || height == 0 { return Err(anyhow!(\"window: width and height must be > 0\")); }\n'
            '                        // allocate RGB buffer (P6 style)\n'
            '                        let buf_size = width.saturating_mul(height).saturating_mul(3);\n'
            '                        let buf = vec![0u8; buf_size];\n'
            '                        let handle = format!(\"win://{}\", self.next_window_id);\n'
            '                        self.next_window_id = self.next_window_id.saturating_add(1);\n'
            '                        self.gfx_windows.insert(handle.clone(), (width, height, buf, true));\n'
            '                        Ok(Value::String(handle))\n'
            '                    }\n'
            '                    _ => Err(anyhow!(\"window: expected (title:string, width:number, height:number)\")),\n'
            '                }\n'
            '            }\n'
        ),
        "first_only": True
    }
]
