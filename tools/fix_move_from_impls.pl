#!/usr/bin/env perl
use strict;
use warnings;
use File::Copy qw(copy);
use Time::Piece;

my $file = "src/interpreter/environment.rs";
die "File not found: $file\n" unless -f $file;

# Backup
my $ts = localtime->strftime('%Y%m%d_%H%M%S');
my $bakdir = ".bak_move_impls_$ts";
mkdir $bakdir or die "Cannot create backup dir $bakdir: $!\n";
copy($file, "$bakdir/environment.rs") or die "Backup failed: $!\n";
print "Backup saved to $bakdir/environment.rs\n";

# Read file
local $/ = undef;
open my $fh, '<', $file or die "Can't open $file: $!\n";
my $text = <$fh>;
close $fh;

# Find the start of the From<f64> impl and the start of fmt::Display impl (insertion anchor)
unless ($text =~ /(impl\s+From<\s*f64\s*>\s*for\s*Value\s*\{.*?\n\})/s) {
    die "Could not find impl From<f64> for Value block. Aborting.\n";
}
my $from_f64_block = $1;

# Remove any impl From<...> for Value blocks that are currently in the file (we will reinsert them)
my @found_impls;
while ($text =~ /(impl\s+From<[^>]+>\s+for\s+Value\s*\{.*?\n\})/sg) {
    push @found_impls, $1;
}
# Keep only unique and not the f64 block (we will reinsert in canonical order)
@found_impls = grep { $_ ne $from_f64_block } @found_impls;

# Remove those impls from the text
foreach my $impl (@found_impls) {
    $text =~ s/\Q$impl\E//s;
}

# Prepare canonical impls (From<f64> already present)
my $extra_impls = <<'EOF';

impl From<i64> for Value {
    fn from(n: i64) -> Self { Value::Number(n as f64) }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self { Value::Number(n as f64) }
}

impl From<usize> for Value {
    fn from(n: usize) -> Self { Value::Number(n as f64) }
}
EOF

# Insert extra_impls after the existing From<f64> impl if possible
if ($text =~ /(impl\s+From<\s*f64\s*>\s*for\s*Value\s*\{.*?\n\})/s) {
    $text =~ s/(impl\s+From<\s*f64\s*>\s*for\s*Value\s*\{.*?\n\})/$1$extra_impls/s;
    print "Inserted From<i64/i32/usize> impls after From<f64> impl.\n";
} elsif ($text =~ /(impl\s+fmt::Display\s+for\s+Value\s*\{)/s) {
    $text =~ s/(impl\s+fmt::Display\s+for\s+Value\s*\{)/$extra_impls$1/s;
    print "Inserted From<i64/i32/usize> impls before fmt::Display impl.\n";
} else {
    $text .= $extra_impls;
    print "Appended From<i64/i32/usize> impls at EOF of environment.rs\n";
}

# Clean up accidental multiple blank lines
$text =~ s/\n{3,}/\n\n/g;

# Write back
open my $out, '>', $file or die "Can't write $file: $!\n";
print $out $text;
close $out;

print "Patched $file. Run: cargo build --release\n";
