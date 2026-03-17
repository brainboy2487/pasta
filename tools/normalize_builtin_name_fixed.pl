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

# Read file into lines
open my $in, '<', $file or die "open in: $!\n";
my @lines = <$in>;
close $in;

# Find the debug println line and insert normalization after it
my $found = 0;
for (my $i = 0; $i < @lines; $i++) {
    if ($lines[$i] =~ /

\[BUILTIN CALL\]

\s*name=/) {
        # Determine indentation of the println line (leading whitespace)
        my ($indent) = ($lines[$i] =~ /^(\s*)/);
        # Insert normalization on the next line
        splice @lines, $i+1, 0, $indent . "let name = name.to_ascii_uppercase();\n";
        $found = 1;
        last;
    }
}

die "Could not find debug println line containing '[BUILTIN CALL] name=' in $file. Make sure you injected the debug print first.\n" unless $found;

# Write back atomically
open my $out, '>', "$file.tmp" or die "open out: $!\n";
print $out @lines;
close $out;
rename("$file.tmp", $file) or die "rename failed: $!\n";
print "Patched $file (inserted name normalization).\n";
