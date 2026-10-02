param([string]$Tag = '')
$ErrorActionPreference = 'Stop'

$package = 'dist/package'
$assets = 'dist/assets'
# A fresh output directory prevents stale files from entering a rerun's release.
if (Test-Path 'dist') { Remove-Item 'dist' -Recurse -Force }
New-Item -ItemType Directory -Path $package,$assets -Force | Out-Null
Copy-Item target/x86_64-pc-windows-msvc/release/wh3-mod-manager.exe "$package/"
# Steam Workshop features run in a worker that loads steam_api64.dll from the
# executable's folder; ship the redistributable that steamworks-sys linked against.
$steamApi = Get-ChildItem 'target/x86_64-pc-windows-msvc/release/build/steamworks-sys-*/out/steam_api64.dll' |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $steamApi) { throw 'steam_api64.dll was not produced by the build.' }
Copy-Item $steamApi.FullName "$package/"
Copy-Item LICENSE,README.md "$package/"
Copy-Item docs -Destination "$package/docs" -Recurse

Get-ChildItem "$package/*.exe","$package/*.dll" | Sort-Object Name | ForEach-Object {
    $hash = Get-FileHash $_.FullName -Algorithm SHA256
    "$($hash.Hash.ToLower())  $($_.Name)"
} | Set-Content "$package/SHA256SUMS.txt" -Encoding utf8

$archive = if ($Tag) { "WH3-Mod-Manager-$Tag-windows-x64.zip" } else { 'WH3-Mod-Manager-windows-x64.zip' }
Compress-Archive -Path "$package/*" -DestinationPath "$assets/$archive"
Copy-Item "$package/*.dll" "$assets/"
# The self-updater requires an asset name different from the installed executable's
# (see EXE_ASSET in crates/core/src/update.rs).
Copy-Item "$package/wh3-mod-manager.exe" "$assets/wh3-mod-manager-windows-x64.exe"
Get-ChildItem "$assets/*" -File -Exclude 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    $hash = Get-FileHash $_.FullName -Algorithm SHA256
    "$($hash.Hash.ToLower())  $($_.Name)"
} | Set-Content "$assets/SHA256SUMS.txt" -Encoding utf8
