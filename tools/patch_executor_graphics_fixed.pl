#!/usr/bin/env perl
use strict;
use warnings;
use File::Copy qw(copy);
use File::Path qw(make_path);
use Time::Piece;

my @targets = ("src/interpreter/executor.rs", "src/interpreter/executor.rs.txt");
my @found = grep { -f $_ } @targets;
die "No executor file found (checked: @targets)\n" unless @found;

# Backup
my $ts = localtime->strftime('%Y%m%d_%H%M%S');
my $bakdir = ".bak_executor_graphics_$ts";
make_path($bakdir);
for my $f (@found) {
    copy($f, "$bakdir/" . ( $f =~ s{.*/}{}r )) or die "Backup failed for $f: $!\n";
    print "Backed up $f -> $bakdir\n";
}

# The code to inject (graphics dispatch)
my $inject = <<'RUST';
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
                    let copy_w = std::cmp::min(*ww, *cw);
                    let copy_h = std::cmp::min(*wh, *ch);
                    for y in 0..copy_h {
                        let dst_row = y * (*ww) * 3;
                        let src_row = y * (*cw) * 3;
                        let bytes = copy_w * 3;
                        wbuf[dst_row..dst_row+bytes].copy_from_slice(&cbuf[src_row..src_row+bytes]);
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

# For each file, load, modify, and write back
for my $file (@found) {
    local $/ = undef;
    open my $in, '<', $file or die "Can't open $file: $!\n";
    my $text = <$in>;
    close $in;

    # 1) Insert new fields after the gfx_canvases declaration
    if ($text =~ s/(pub\s+gfx_canvases\s*:\s*HashMap<\s*String\s*,\s*\(usize,\s*usize,\s*Vec<u8>\)\s*>\s*,\s*\n)/$1            pub gfx_windows: std::collections::HashMap<String, (usize, usize, Vec<u8>, bool)>,\n            pub next_window_id: usize,\n/s) {
        print "Inserted gfx_windows/next_window_id fields into $file\n";
    } else {
        warn "Did not find gfx_canvases declaration in $file; skipping field insertion\n";
    }

    # 2) Initialize new fields in Executor::new (after next_canvas_id: 1,)
    if ($text =~ s/(next_canvas_id:\s*1,\s*\n)/$1            gfx_windows: std::collections::HashMap::new(),\n            next_window_id: 1,\n/s) {
        print "Initialized gfx_windows/next_window_id in new() for $file\n";
    } else {
        warn "Did not find next_canvas_id initializer in $file; skipping new() init\n";
    }

    # 3) Inject graphics dispatch before the first 'match name {' occurrence
    if ($text =~ s/(\bmatch\s+name\s*\{)/$inject$1/s) {
        print "Injected graphics dispatch into $file\n";
    } else {
        warn "Did not find 'match name {' in $file; skipping dispatch injection\n";
    }

    # Write back atomically
    my $tmp = "$file.tmp";
    open my $out, '>', $tmp or die "Can't write $tmp: $!\n";
    print $out $text;
    close $out;
    rename($tmp, $file) or die "Can't move $tmp to $file: $!\n";
    print "Patched $file\n";
}

print "All done. Run 'cargo build' and fix any borrow/ownership issues if the compiler reports them.\n";
