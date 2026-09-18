#!/usr/bin/env bash
# split_for_upload_no_compress.sh
# Split large text files into numbered chunks placed in the same directory as the source file.
# Usage:
#   ./split_for_upload_no_compress.sh [-l LINES] [-b BYTES] [-n CHUNKS] file1 [file2 ...]
# Options (mutually exclusive):
#   -l LINES   Split by number of lines per chunk (default: 2000)
#   -b BYTES   Split by approximate bytes per chunk (e.g., 100K, 2M)
#   -n CHUNKS  Split into N roughly-equal chunks (line-based)
# Examples:
#   ./split_for_upload_no_compress.sh -l 2000 docs/README.txt
#   ./split_for_upload_no_compress.sh -b 200K src/large_doc.txt
#   ./split_for_upload_no_compress.sh -n 5 docs/*.md

set -euo pipefail

LINES_PER_CHUNK=2000
BYTES_PER_CHUNK=""
NUM_CHUNKS=0

print_usage() {
  cat <<EOF
Usage: $0 [-l LINES] [-b BYTES] [-n CHUNKS] file1 [file2 ...]
  -l LINES   Split by lines per chunk (default: ${LINES_PER_CHUNK})
  -b BYTES   Split by approximate bytes per chunk (supports K/M suffix)
  -n CHUNKS  Split into N roughly-equal chunks (line-based)
Note: -l, -b and -n are mutually exclusive.
EOF
}

while getopts ":l:b:n:h" opt; do
  case $opt in
    l) LINES_PER_CHUNK="$OPTARG" ;;
    b) BYTES_PER_CHUNK="$OPTARG" ;;
    n) NUM_CHUNKS="$OPTARG" ;;
    h) print_usage; exit 0 ;;
    \?) echo "Invalid option: -$OPTARG" >&2; print_usage; exit 1 ;;
    :) echo "Option -$OPTARG requires an argument." >&2; print_usage; exit 1 ;;
  esac
done
shift $((OPTIND -1))

if [ "$#" -lt 1 ]; then
  echo "Error: at least one file must be provided." >&2
  print_usage
  exit 1
fi

if [ -n "$BYTES_PER_CHUNK" ] && [ "$NUM_CHUNKS" -ne 0 ]; then
  echo "Error: -b and -n are mutually exclusive." >&2
  exit 1
fi
if [ -n "$BYTES_PER_CHUNK" ] && [ "$LINES_PER_CHUNK" != "2000" ]; then
  echo "Error: -b and -l are mutually exclusive." >&2
  exit 1
fi
if [ "$NUM_CHUNKS" -ne 0 ] && [ "$LINES_PER_CHUNK" != "2000" ]; then
  echo "Error: -n and -l are mutually exclusive." >&2
  exit 1
fi

for src in "$@"; do
  if [ ! -f "$src" ]; then
    echo "Skipping: $src (not a regular file)"
    continue
  fi

  dir="$(dirname -- "$src")"
  base="$(basename -- "$src")"
  name="${base%.*}"
  ext="${base##*.}"
  if [ "$name" = "$base" ]; then
    # no extension
    ext="txt"
  fi

  echo "Processing: $src"
  echo "Output directory: $dir"

  tmp_prefix="${dir}/.${name}.split."
  rm -f "${tmp_prefix}"* 2>/dev/null || true

  if [ "$NUM_CHUNKS" -gt 0 ]; then
    total_lines=$(wc -l < "$src" || echo 0)
    if [ "$total_lines" -eq 0 ]; then
      outname="${name}.part01.${ext}"
      cp -- "$src" "${dir}/${outname}"
      printf "%s\n" "${outname}" > "${dir}/${name}.manifest.txt"
      echo "Wrote single chunk: ${outname}"
      continue
    fi
    lines_per_chunk=$(( (total_lines + NUM_CHUNKS - 1) / NUM_CHUNKS ))
    echo "Total lines: $total_lines; splitting into $NUM_CHUNKS chunks (~$lines_per_chunk lines each)"
    split -d -a 2 -l "$lines_per_chunk" --additional-suffix=".tmp" "$src" "${tmp_prefix}"
  elif [ -n "$BYTES_PER_CHUNK" ]; then
    bytes="$BYTES_PER_CHUNK"
    if split --version >/dev/null 2>&1 && split -C 1 >/dev/null 2>&1 2>/dev/null; then
      echo "Splitting by approx bytes: $bytes (using split -C)"
      split -d -a 2 -C "$bytes" --additional-suffix=".tmp" "$src" "${tmp_prefix}"
    else
      echo "Splitting by bytes: $bytes (using split -b; may cut long lines)"
      split -d -a 2 -b "$bytes" --additional-suffix=".tmp" "$src" "${tmp_prefix}"
    fi
  else
    echo "Splitting by lines: $LINES_PER_CHUNK lines per chunk"
    split -d -a 2 -l "$LINES_PER_CHUNK" --additional-suffix=".tmp" "$src" "${tmp_prefix}"
  fi

  manifest="${dir}/${name}.manifest.txt"
  : > "$manifest"

  idx=1
  # ensure sorted order
  for tmp in $(ls -1 "${tmp_prefix}"* 2>/dev/null | sort); do
    printf -v num "%02d" "$idx"
    outname="${name}.part${num}.${ext}"
    mv -f -- "$tmp" "${dir}/${outname}"
    echo "${outname}" >> "$manifest"
    idx=$((idx + 1))
  done

  if [ "$idx" -eq 1 ]; then
    # No parts created; copy original as single chunk
    outname="${name}.part01.${ext}"
    cp -- "$src" "${dir}/${outname}"
    echo "${outname}" >> "$manifest"
    echo "Created single chunk: ${outname}"
  fi

  echo "Wrote $((idx - 1)) chunk(s). Manifest: $manifest"
done

echo "Done."
