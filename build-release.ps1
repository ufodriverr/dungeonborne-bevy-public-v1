param([switch]$Offline)

$ErrorActionPreference = 'Stop'
$previousFlags = $env:CARGO_ENCODED_RUSTFLAGS
Push-Location $PSScriptRoot
try {
    # Keep the local user profile and this folder out of compiled source-location strings.
    $env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$PSScriptRoot=/src" + [char]0x1f + "--remap-path-prefix=$env:USERPROFILE=/build"
    $cargoArgs = @('build', '--locked', '--release', '-p', 'db_game')
    if ($Offline) { $cargoArgs += '--offline' }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }

    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    New-Item -ItemType Directory -Force dist | Out-Null
    $zip = "dist/dungeonborne-bevy-$version-windows-x64.zip"
    # Only the executable and the notices (with the crates' licence texts): never game data or
    # local files.
    $files = @('README.md', 'HOW-TO-PLAY.md', 'EXTRACTION.md', 'LEGAL.md', 'CREDITS.md', 'RELEASE-NOTES.md', 'LICENSE',
        'THIRD-PARTY-NOTICES.md', 'licenses')
    # Windows' own bsdtar writes a ZIP with forward-slash entry names (Compress-Archive on
    # PowerShell 5.1 writes backslashes, which some unzip tools mishandle); the exe at the root.
    & "$env:SystemRoot\System32\tar.exe" -a -c -f $zip -C 'target\release' 'dungeonborne.exe' -C $PSScriptRoot @files
    if ($LASTEXITCODE -ne 0) { throw "tar failed with exit code $LASTEXITCODE" }
    Write-Output "release: $zip"
}
finally {
    $env:CARGO_ENCODED_RUSTFLAGS = $previousFlags
    Pop-Location
}
