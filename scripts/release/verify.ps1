Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "==> Verifying release artifact checksum (Windows)"

$checksumPath = "dist\pasta-windows-x64.sha256"
$archivePath = "dist\pasta-windows-x64.zip"
if (-not (Test-Path $checksumPath)) { throw "missing checksum file: $checksumPath" }
if (-not (Test-Path $archivePath)) { throw "missing archive file: $archivePath" }

$line = (Get-Content $checksumPath -Raw).Trim()
$expected = ($line -split '\s+')[0].ToLower()
$actual = (Get-FileHash $archivePath -Algorithm SHA256).Hash.ToLower()
if ($expected -ne $actual) {
    throw "checksum mismatch: expected $expected, got $actual"
}

if (-not (Test-Path "dist\release-manifest.txt")) {
    throw "missing release manifest: dist\\release-manifest.txt"
}

Write-Host "RELEASE_VERIFY_OK"
