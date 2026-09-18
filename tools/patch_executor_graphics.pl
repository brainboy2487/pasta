#!/usr/bin/env perl
use strict;
use warnings;
use File::Copy qw(copy);
use File::Path qw(make_path);
use Time::Piece;

my @targets = ("src/interpreter/executor.rs", "src/interpreter/executor.rs.txt");
my @found;
for my $f (@targets) { push @found, $f if -f $f }

die "No executor file found (checked: @targets)\n" unless @found;

# Backup
my $ts = localtime->strftime('%Y%m%d_%H%M%S');
my $bakdir = ".bak_executor_graphics_$ts";
make_path($bakdir);
for my $f (@found) {
    copy($f, "$bakdir/" . ( $f =~ s{.*/}{}r )) or die "Backup failed for $f: $!\n";
    print "Backed up $f -> $bakdir\n";
}

# 1) Insert new fields into the Executor struct (after gfx_canvases line)
for my $file (@found) {
    local @ARGV = ($file);
    local $^I = ".tmp";
    my $inserted_fields = 0;
    while (<>) {
        print;
        if (!$inserted_fields && /pub\s+gfx_canvases\s*:\s*HashMap<\s*String\s*,\s*\(usize,\s*usize,\s*Vec<u8>\)\s*>\s*,/) {
            print "            pub gfx_windows: std::collections::HashMap<String, (usize, usize, Vec<u8>, bool)>,\n";
            print "            pub next_window_id: usize,\n";
            $inserted_fields = 1;
        }
    }
    unlink("$file.tmp");
    rename("$file.tmp", $file) or die "rename failed: $!\n" unless -f "$file.tmp";
}

# 2) Update Executor::new() to initialize the new fields
for my $file (@found) {
    local @ARGV = ($file);
    local $^I = ".tmp2";
    my $in_new = 0;
    my $replaced = 0;
    while (<>) {
        if (/pub\s+fn\s+new\(\)\s*->\s*Self\s*\{/) { $in_new = 1; print; next; }
        if ($in_new && /next_canvas_id:\s*1,/) {
            print $_;
            print "            gfx_windows: std::collections::HashMap::new(),\n";
            print "            next_window_id: 1,\n";
            $replaced = 1;
            next;
        }
        if ($in_new && /\}\s*;\s*$/) { $in_new = 0; }
        print;
    }
    unlink("$file.tmp2");
    rename("$file.tmp2", $file) or die "rename failed: $!\n" unless -f "$file.tmp2";
}

# 3) Inject graphics builtin dispatch before the main `match name {` in call_builtin
my $injected_code = <<'RUST';
        // ── Graphics builtins (WINDOW, CANVAS, PIXEL, BLIT, WINDOW_OPEN, CLOSE)
        // Handles are returned as strings: "canvas://<id>" and "window://<id>"
        if name == "CANVAS" {
            if args.len() != 2 { return Err(anyhow!("CANVAS expects 2 args (width, height)")); }
            match (&args[0], &args[1]) {
                (Value::Number(w), Value::Number(h)) => {
                    let width = *w as usize;
                    let height = *h as usize;
                    let id = format!("canvas://{}", self.next_canvas_id);
                    self.next_canvas_id = self.next_canvas_id.saturating_add(1);
                    // RGB buffer: width * height * 3
                    let buf = vec![0u8; width.saturating_mul(height).saturating_mul(3)];
                    self.gfx_canvases.insert(id.clone(), (width, height, buf));
                    return Ok(Value::String(id));
                }
                _ => return Err(anyhow!("CANVAS: width and height must be numbers")),
            }
        }
        if name == "WINDOW" {
            if args.len() != 3 { return Err(anyhow!("WINDOW expects 3 args (title, width, height)")); }
            match (&args[0], &args[1], &args[2]) {
                (Value::String(_title), Value::Number(w), Value::Number(h)) => {
                    let width = *w as usize;
                    let height = *h as usize;
                    let id = format!("window://{}", self.next_window_id);
                    self.next_window_id = self.next_window_id.saturating_add(1);
                    // Window buffer same layout as canvas; `true` = open
                    let buf = vec![0u8; width.saturating_mul(height).saturating_mul(3)];
                    self.gfx_windows.insert(id.clone(), (width, height, buf, true));
                    return Ok(Value::String(id));
                }
                _ => return Err(anyhow!("WINDOW: expected (title:string, width:number, height:number)")),
            }
        }
        if name == "PIXEL" {
            if args.len() != 6 { return Err(anyhow!("PIXEL expects 6 args (canvas_handle, x, y, r, g, b)")); }
            match (&args[0], &args[1], &args[2], &args[3], &args[4], &args[5]) {
                (Value::String(handle), Value::Number(xn), Value::Number(yn), Value::Number(rn), Value::Number(gn), Value::Number(bn)) => {
                    if !handle.starts_with("canvas://") { return Err(anyhow!("PIXEL: first arg must be a canvas handle")); }
                    let (x, y) = (*xn as isize, *yn as isize);
                    let (r, g, b) = (*rn as i64 as u8, *gn as i64 as u8, *bn as i64 as u8);
                    let entry = match self.gfx_canvases.get_mut(handle) {
                        Some(e) => e,
                        None => return Err(anyhow!("PIXEL: unknown canvas handle")),
                    };
                    let (w, h, buf) = entry;
                    if x < 0 || y < 0 || x as usize >= *w || y as usize >= *h { return Err(anyhow!("PIXEL: coordinates out of bounds")); }
                    let idx = (y as usize * *w + x as usize) * 3;
                    if idx + 2 >= buf.len() { return Err(anyhow!("PIXEL: internal buffer error")); }
                    buf[idx] = r;
                    buf[idx+1] = g;
                    buf[idx+2] = b;
                    return Ok(Value::None);
                }
                _ => return Err(anyhow!("PIXEL: invalid argument types")),
            }
        }
        if name == "BLIT" {
            if args.len() != 2 { return Err(anyhow!("BLIT expects 2 args (window_handle, canvas_handle)")); }
            match (&args[0], &args[1]) {
                (Value::String(win_h), Value::String(can_h)) => {
                    let win = match self.gfx_windows.get_mut(win_h) {
                        Some(w) => w,
                        None => return Err(anyhow!("BLIT: unknown window handle")),
                    };
                    let can = match self.gfx_canvases.get(can_h) {
                        Some(c) => c,
                        None => return Err(anyhow!("BLIT: unknown canvas handle")),
                    };
                    let (ww, wh, wbuf, open) = win;
                    if !*open { return Err(anyhow!("BLIT: window is closed")); }
                    let (cw, ch, cbuf) = can;
                    // Simple blit: copy min(width,height) region
                    let copy_w = std::cmp::min(*ww, *cw);
                    let copy_h = std::cmp::min(*wh, *ch);
                    for y in 0..copy_h {
                        let dst_row = y * (*ww) * 3;
                        let src_row = y * (*cw) * 3;
                        let dst_idx = dst_row;
                        let src_idx = src_row;
                        let bytes = copy_w * 3;
                        wbuf[dst_idx..dst_idx+bytes].copy_from_slice(&cbuf[src_idx..src_idx+bytes]);
                    }
                    return Ok(Value::None);
                }
                _ => return Err(anyhow!("BLIT: expected two string handles")),
            }
        }
        if name == "WINDOW_OPEN" {
            if args.len() != 1 { return Err(anyhow!("WINDOW_OPEN expects 1 arg (window_handle)")); }
            match &args[0] {
                Value::String(h) => {
                    match self.gfx_windows.get(h) {
                        Some((_w,_h,_buf,open)) => return Ok(Value::Bool(*open)),
                        None => return Err(anyhow!("WINDOW_OPEN: unknown window handle")),
                    }
                }
                _ => return Err(anyhow!("WINDOW_OPEN: expected window handle string")),
            }
        }
        if name == "CLOSE" {
            if args.len() != 1 { return Err(anyhow!("CLOSE expects 1 arg (window_handle)")); }
            match &args[0] {
                Value::String(h) => {
                    match self.gfx_windows.get_mut(h) {
                        Some((_w,_h,_buf,open)) => { *open = false; return Ok(Value::None); }
                        None => return Err(anyhow!("CLOSE: unknown window handle")),
                    }
                }
                _ => return Err(anyhow!("CLOSE: expected window handle string")),
            }
        }
RUST

for my $file (@found) {
    local @ARGV = ($file);
    local $^I = ".tmp3";
    my $injected = 0;
    while (<>) {
        if (!$injected && /match\s+name\s*\{/) {
            print $injected_code;
            $injected = 1;
        }
        print;
    }
    unlink("$file.tmp3");
    rename("$file.tmp3", $file) or die "rename failed: $!\n" unless -f "$file.tmp3";
    print "Injected graphics dispatch into $file\n";
}

print "Done. Please run 'cargo build' and fix any borrow/lifetime issues if your toolchain flags them.\n";
