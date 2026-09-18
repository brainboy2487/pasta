Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Needle,
        [Parameter(Mandatory = $true)][string]$Context
    )
    if ($Text -notmatch [Regex]::Escape($Needle)) {
        throw "$Context missing required marker: $Needle"
    }
}

Write-Host "==> Phase 6 release checklist (Windows)"

cargo build --workspace --all-targets --locked
cargo test --workspace --all --release --locked
cargo build --release --locked

$sortOut = .\target\release\pasta.exe tests\phase5_sort_smoke.ps | Out-String
Assert-Contains -Text $sortOut -Needle "=== PHASE5 SORT SMOKE PASS ===" -Context "phase5_sort_smoke.ps"

$fullOut = .\target\release\pasta.exe tests\10_full_suite.ps | Out-String
Assert-Contains -Text $fullOut -Needle "=== ALL 50 TESTS COMPLETE ===" -Context "10_full_suite.ps"

$bigOut = .\target\release\pasta.exe tests\09_big_test.ps | Out-String
Assert-Contains -Text $bigOut -Needle "=== ALL TESTS COMPLETE ===" -Context "09_big_test.ps"

Write-Host "RELEASE_CHECKLIST_OK"
