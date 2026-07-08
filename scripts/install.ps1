$ErrorActionPreference = "Stop"

$Repo = if ($env:BURNLY_REPO) { $env:BURNLY_REPO } else { "fikrilal/burnly" }
$Version = if ($env:BURNLY_VERSION) { $env:BURNLY_VERSION } else { "latest" }

$Architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x86_64" }
    "x86_64" { "x86_64" }
    default {
        throw "Unsupported Windows architecture: $env:PROCESSOR_ARCHITECTURE. Burnly currently publishes a Windows x64 installer."
    }
}

if ($Version -eq "latest") {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $Release.tag_name
    if (-not $Version) {
        throw "Could not resolve the latest Burnly release."
    }
    $ReleaseBaseUrl = "https://github.com/$Repo/releases/latest/download"
} else {
    $ReleaseBaseUrl = "https://github.com/$Repo/releases/download/$Version"
}

if ($Version -notmatch "^v(.+)$") {
    throw "Burnly release tag must use the vX.Y.Z format: $Version"
}

$ReleaseVersion = $Matches[1]
$AssetName = "burnly-v$ReleaseVersion-windows-$Architecture.exe"
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("burnly-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempDir | Out-Null

try {
    $InstallerPath = Join-Path $TempDir $AssetName
    $ChecksumPath = Join-Path $TempDir "SHA256SUMS"

    Write-Host "Downloading Burnly $Version for windows-$Architecture..."
    Invoke-WebRequest -Uri "$ReleaseBaseUrl/$AssetName" -OutFile $InstallerPath
    Invoke-WebRequest -Uri "$ReleaseBaseUrl/SHA256SUMS" -OutFile $ChecksumPath

    $ChecksumLine = Get-Content $ChecksumPath | Where-Object {
        $_ -match "^\s*([a-fA-F0-9]{64})\s+(?:artifacts/)?$([regex]::Escape($AssetName))$"
    } | Select-Object -First 1

    if (-not $ChecksumLine) {
        throw "SHA256SUMS does not contain $AssetName"
    }

    if ($ChecksumLine -notmatch "^\s*([a-fA-F0-9]{64})\s+") {
        throw "Could not parse SHA256SUMS entry for $AssetName"
    }

    $ExpectedSha256 = $Matches[1].ToLowerInvariant()
    $ActualSha256 = (Get-FileHash -Algorithm SHA256 -Path $InstallerPath).Hash.ToLowerInvariant()
    if ($ActualSha256 -ne $ExpectedSha256) {
        throw "Checksum verification failed for $AssetName. Expected $ExpectedSha256, got $ActualSha256."
    }

    Write-Host "Starting Burnly installer..."
    $Process = Start-Process -FilePath $InstallerPath -Wait -PassThru
    if ($Process.ExitCode -ne 0) {
        throw "Burnly installer exited with code $($Process.ExitCode)."
    }

    Write-Host "Burnly installer completed."
} finally {
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
