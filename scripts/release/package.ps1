Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "==> Packaging release artifact (Windows)"

New-Item -ItemType Directory -Path dist -Force | Out-Null
Copy-Item target\release\pasta.exe dist\ -Force
Compress-Archive -Path dist\pasta.exe -DestinationPath dist\pasta-windows-x64.zip -Force

$hash = (Get-FileHash dist\pasta-windows-x64.zip -Algorithm SHA256).Hash.ToLower()
"$hash  pasta-windows-x64.zip" | Out-File -FilePath dist\pasta-windows-x64.sha256 -Encoding ascii

$cargoVersion = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.*)"').Matches[0].Groups[1].Value
$createdUtc = (Get-Date).ToUniversalTime().ToString("o")
@"
os=windows
artifact=pasta-windows-x64.zip
checksum_file=pasta-windows-x64.sha256
cargo_version=$cargoVersion
created_utc=$createdUtc
"@ | Out-File -FilePath dist\release-manifest.txt -Encoding ascii

Write-Host "RELEASE_PACKAGE_OK"
