#!/usr/bin/env perl
use strict;
use warnings;
use File::Copy qw(copy);
use Time::Piece;

my $file = "src/interpreter/executor.rs";
die "File not found: $file\n" unless -f $file;

# Backup
my $ts = localtime->strftime('%Y%m%d_%H%M%S');
copy($file, "$file.bak_$ts") or die "Backup failed: $!\n";
print "Backup: $file.bak_$ts\n";

open my $in, '<', $file or die "open in: $!\n";
local $/ = undef;
my $text = <$in>;
close $in;

my $save_code = <<'RUST';
        if name == "WINDOW_SAVE" {
            if args.len() != 2 { return Err(anyhow!("WINDOW_SAVE expects 2 args (window_handle, path)")); }
            match (&args[0], &args[1]) {
                (Value::String(h), Value::String(path)) => {
                    let win = match self.gfx_windows.get(h) {
                        Some(w) => w,
                        None => return Err(anyhow!("WINDOW_SAVE: unknown window handle")),
                    };
                    let (w, hgt, buf, _open) = win;
                    // Write P6 PPM
                    use std::fs::File;
                    use std::io::Write;
                    let mut f = File::create(path).map_err(|e| anyhow!("WINDOW_SAVE: {}", e))?;
                    let header = format!("P6\n{} {}\n255\n", *w, *hgt);
                    f.write_all(header.as_bytes()).map_err(|e| anyhow!("WINDOW_SAVE: {}", e))?;
                    f.write_all(&buf).map_err(|e| anyhow!("WINDOW_SAVE: {}", e))?;
                    return Ok(Value::None);
                }
                _ => return Err(anyhow!("WINDOW_SAVE: expected (window_handle:string, path:string)")),
            }
        }
RUST

# Insert before the first 'if name == "WINDOW_OPEN"' if present, otherwise before 'match name {'
if ($text =~ s/(\n\s*if\s+name\s*==\s*"WINDOW_OPEN")/$save_code$1/s) {
    print "Inserted WINDOW_SAVE before WINDOW_OPEN\n";
} elsif ($text =~ s/(\bmatch\s+name\s*\{)/$save_code$1/s) {
    print "Inserted WINDOW_SAVE before match name\n";
} else {
    die "Could not find injection point in $file\n";
}

open my $out, '>', $file or die "open out: $!\n";
print $out $text;
close $out;
print "Patched $file\n";
