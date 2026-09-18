#!/usr/bin/env perl
use strict;
use warnings;
use File::Copy qw(copy);
use Time::Piece;

my $file = "src/interpreter/executor.rs";
die "File not found: $file\n" unless -f $file;

# Backup
my $ts = localtime->strftime('%Y%m%d_%H%M%S');
my $bak = "$file.bak_$ts";
copy($file, $bak) or die "Backup failed: $!\n";
print "Backup created: $bak\n";

# Read file
local $/ = undef;
open my $in, '<', $file or die "open in: $!\n";
my $text = <$in>;
close $in;

# Alias snippet to insert (Rust code). Inserted after each functions.insert(...) call.
my $alias_snippet = <<'RUST';
/* Inserted alias registration: if the function name contains a dot (e.g., "def.swap"),
   also register the short name ("swap") so calls using either form resolve. */
if let Some(pos) = name.name.rfind('.') {
    let short = name.name[pos+1..].to_string();
    if !self.functions.contains_key(&short) {
        self.functions.insert(short.clone(), (params.clone(), body.clone()));
    }
    if params.is_empty() {
        self.env.set_global(short, Value::Lambda(body.clone()));
    }
}
RUST

# Perform a global insertion: after every occurrence of the functions.insert(...) line,
# add the alias snippet. This is conservative and reversible (backup created).
my $anchor = qr/self\.functions\.insert\(\s*name\.name\.clone\(\)\s*,\s*\(params\.clone\(\)\s*,\s*body\.clone\(\)\)\s*\)\s*;/;
my $count = 0;
$text =~ s/($anchor)/$1\n$alias_snippet/s and $count = () = ($text =~ /$anchor/g);

if ($count == 0) {
    print "Warning: no occurrences of the functions.insert anchor were found. Restoring backup and aborting.\n";
    copy($bak, $file) or die "Restore failed: $!\n";
    exit 1;
}

# Write back atomically
open my $out, '>', "$file.tmp" or die "open out: $!\n";
print $out $text;
close $out;
rename("$file.tmp", $file) or die "rename failed: $!\n";
print "Patched $file (inserted alias snippet after $count occurrence(s)).\n";

# Rebuild
print "Running: cargo build --release\n";
my $build_out = qx{cargo build --release 2>&1};
my $build_status = $? >> 8;
print $build_out;

if ($build_status != 0) {
    print "Build failed (exit $build_status). Writing build log to EATME.txt\n";
    open my $log, '>', "EATME.txt" or die "Cannot write EATME.txt: $!\n";
    print $log "=== BUILD OUTPUT ===\n$build_out\n";
    close $log;
    print "Saved EATME.txt (build failure). Provide it for the next iteration.\n";
    exit $build_status;
}

# Run the bubble sort test
my $test_cmd = "./target/release/pasta tests/bubble_sort.ps";
print "Running test: $test_cmd\n";
my $run_out = qx{$test_cmd 2>&1};
my $run_status = $? >> 8;
print $run_out;

if ($run_status != 0) {
    print "Test failed (exit $run_status). Saving output to EATME.txt\n";
    open my $log, '>', "EATME.txt" or die "Cannot write EATME.txt: $!\n";
    print $log "=== BUILD OUTPUT ===\n$build_out\n\n=== TEST OUTPUT ===\n$run_out\n";
    close $log;
    print "Saved EATME.txt. Provide it to the agent for the next patch.\n";
    exit $run_status;
} else {
    print "Test succeeded. No EATME.txt created.\n";
    exit 0;
}
