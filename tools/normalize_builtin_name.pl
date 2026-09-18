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

# Find the debug print we injected earlier and insert normalization after it.
# We look for the exact debug print line and insert the uppercase normalization.
my $needle = qr/\n\s*println!\(\s*"

\[BUILTIN CALL\]

 name='{}' args=\{\:?\?\}",\s*name,\s*args\s*\);\s*\n/s;

if ($text =~ /$needle/) {
    $text =~ s/($needle)/$1    // Normalize builtin name to uppercase so comparisons match\n    let name = name.to_ascii_uppercase();\n/s;
    print "Inserted name normalization after debug print\n";
} else {
    # If the exact debug print isn't present, try a more permissive match for the println line
    if ($text =~ s/(\n\s*println!\([^\n]*

\[BUILTIN CALL\]

[^\n]*\);\s*\n)/$1    // Normalize builtin name to uppercase so comparisons match\n    let name = name.to_ascii_uppercase();\n/s) {
        print "Inserted name normalization after a permissive debug print match\n";
    } else {
        die "Could not find the debug print line in $file. Make sure you injected the debug print first.\n";
    }
}

# Write back
open my $out, '>', "$file.tmp" or die "open out: $!\n";
print $out $text;
close $out;
rename("$file.tmp", $file) or die "rename failed: $!\n";
print "Patched $file\n";

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
