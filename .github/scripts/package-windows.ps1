param([string]$Tag = '')
$ErrorActionPreference = 'Stop'

$package = 'dist/package'
$assets = 'dist/assets'
# A fresh output directory prevents stale files from entering a rerun's release.
if (Test-Path 'dist') { Remove-Item 'dist' -Recurse -Force }
New-Item -ItemType Directory -Path $package,$assets -Force | Out-Null
Copy-Item target/x86_64-pc-windows-msvc/release/wh3-mod-manager.exe "$package/"
Copy-Item LICENSE,README.md "$package/"
Copy-Item docs -Destination "$package/docs" -Recurse

Get-ChildItem "$package/*.exe" | Sort-Object Name | ForEach-Object {
    $hash = Get-FileHash $_.FullName -Algorithm SHA256
    "$($hash.Hash.ToLower())  $($_.Name)"
} | Set-Content "$package/SHA256SUMS.txt" -Encoding utf8

$archive = if ($Tag) { "WH3-Mod-Manager-$Tag-windows-x64.zip" } else { 'WH3-Mod-Manager-windows-x64.zip' }
Compress-Archive -Path "$package/*" -DestinationPath "$assets/$archive"
Copy-Item "$package/*.exe" "$assets/"
Get-ChildItem "$assets/*" -File -Exclude 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    $hash = Get-FileHash $_.FullName -Algorithm SHA256
    "$($hash.Hash.ToLower())  $($_.Name)"
} | Set-Content "$assets/SHA256SUMS.txt" -Encoding utf8
