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

# Find the start of call_builtin and inject the println as the first statement.
# This looks for the function header "pub fn call_builtin" and the following opening brace.
# It inserts the debug print immediately after the brace.
if ($text =~ s/(pub\s+fn\s+call_builtin\s*\([^)]*\)\s*->\s*Result<Value>\s*\{\s*)/$1    println!("[BUILTIN CALL] name='{}' args={:?}", name, args);\n/s) {
    print "Injected debug print into call_builtin\n";
} else {
    die "Could not find 'pub fn call_builtin(...) -> Result<Value> {'. Aborting.\n";
}

# Write back
open my $out, '>', "$file.tmp" or die "open out: $!\n";
print $out $text;
close $out;
rename("$file.tmp", $file) or die "rename failed: $!\n";
print "Patched $file\n";
