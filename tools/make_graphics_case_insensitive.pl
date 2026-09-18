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
print "Backup created: $file.bak_$ts\n";

# Read file
local $/ = undef;
open my $in, '<', $file or die "open in: $!\n";
my $text = <$in>;
close $in;

# Replace equality checks with eq_ignore_ascii_case
$text =~ s/\bif\s+name\s*==\s*"CANVAS"/if name.eq_ignore_ascii_case("CANVAS")/g;
$text =~ s/\bif\s+name\s*==\s*"WINDOW"/if name.eq_ignore_ascii_case("WINDOW")/g;
$text =~ s/\bif\s+name\s*==\s*"PIXEL"/if name.eq_ignore_ascii_case("PIXEL")/g;
$text =~ s/\bif\s+name\s*==\s*"BLIT"/if name.eq_ignore_ascii_case("BLIT")/g;
$text =~ s/\bif\s+name\s*==\s*"WINDOW_OPEN"/if name.eq_ignore_ascii_case("WINDOW_OPEN")/g;
$text =~ s/\bif\s+name\s*==\s*"CLOSE"/if name.eq_ignore_ascii_case("CLOSE")/g;
$text =~ s/\bif\s+name\s*==\s*"WINDOW_SAVE"/if name.eq_ignore_ascii_case("WINDOW_SAVE")/g;

# Write back
open my $out, '>', "$file.tmp" or die "open out: $!\n";
print $out $text;
close $out;
rename("$file.tmp", $file) or die "rename failed: $!\n";
print "Patched $file (case-insensitive checks inserted)\n";

# Rebuild
print "Running cargo build --release ...\n";
system("cargo build --release") == 0 or die "cargo build failed\n";

# Run the test if it exists
my $test = "tests/test_graphics.pasta";
if (-f $test) {
    print "Running: ./target/release/pasta $test\n";
    system("./target/release/pasta $test");
} else {
    print "Test file $test not found; patch applied and project built.\n";
}
