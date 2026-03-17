param(
  [string]$RepoUrl = "https://github.com/godot-rust/gdext.git",
  [string]$Branch = "master",
  [switch]$DryRun,
  [switch]$SkipLockfile
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-CommandExists([string]$CommandName) {
  $cmd = Get-Command $CommandName -ErrorAction SilentlyContinue
  if (-not $cmd) {
    throw "Command not found: '$CommandName'. Ensure it is installed and on PATH."
  }
}

function Try-GetCommand([string]$CommandName) {
  return Get-Command $CommandName -ErrorAction SilentlyContinue
}

function Get-GitHubApiCommitUrl([string]$RepositoryUrl, [string]$BranchName) {
  if ($RepositoryUrl -match '^https://github\.com/(?<owner>[^/]+)/(?<repo>[^/.]+?)(?:\.git)?/?$') {
    return "https://api.github.com/repos/$($Matches.owner)/$($Matches.repo)/commits/$BranchName"
  }

  if ($RepositoryUrl -match '^git@github\.com:(?<owner>[^/]+)/(?<repo>[^/.]+?)(?:\.git)?$') {
    return "https://api.github.com/repos/$($Matches.owner)/$($Matches.repo)/commits/$BranchName"
  }

  return $null
}

function Get-LatestGitRev([string]$RepositoryUrl, [string]$BranchName) {
  $refName = "refs/heads/$BranchName"
  $apiUrl = Get-GitHubApiCommitUrl -RepositoryUrl $RepositoryUrl -BranchName $BranchName

  if ($apiUrl) {
    try {
      $response = Invoke-RestMethod -Headers @{ 'User-Agent' = 'p1proto-update-script' } -Uri $apiUrl
      $rev = $response.sha
      if ($rev -match '^[0-9a-f]{40}$') {
        return $rev
      }
      throw "Unexpected rev returned by GitHub API: '$rev'"
    }
    catch {
      Write-Warning "GitHub API lookup failed, falling back to git ls-remote. $_"
    }
  }

  $git = Try-GetCommand "git"
  if ($git) {
    $hadNativePreference = Test-Path variable:PSNativeCommandUseErrorActionPreference
    if ($hadNativePreference) {
      $previousNativePreference = $PSNativeCommandUseErrorActionPreference
      $script:PSNativeCommandUseErrorActionPreference = $false
    }

    try {
      $output = & git ls-remote $RepositoryUrl $refName 2>$null
    } finally {
      if ($hadNativePreference) {
        $script:PSNativeCommandUseErrorActionPreference = $previousNativePreference
      }
    }

    if ((Test-Path variable:LASTEXITCODE) -and $LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($output)) {
      $rev = ($output -split "\s+")[0].Trim()
      if ($rev -match '^[0-9a-f]{40}$') {
        return $rev
      }
    }
  }

  throw "Could not resolve latest rev for '$RepositoryUrl' '$refName'."
}

function Get-GodotRevMatch([string]$CargoTomlPath) {
  $content = Get-Content -Raw -LiteralPath $CargoTomlPath
  $pattern = '(?ms)(?<prefix>\bgodot\s*=\s*\{)(?<body>[^}]*)(?<suffix>\})'
  $match = [regex]::Match($content, $pattern)
  if (-not $match.Success) {
    throw "Could not find a 'godot' dependency inline table in $CargoTomlPath"
  }

  $body = $match.Groups['body'].Value
  if ($body -notmatch '\bgit\s*=\s*"(?<git>[^"]+)"') {
    throw "Could not find a git-based 'godot' dependency in $CargoTomlPath"
  }

  $revMatch = [regex]::Match($body, '\brev\s*=\s*"(?<rev>[0-9a-f]{40})"')

  return @{
    Content = $content
    Match = $match
    Body = $body
    GitUrl = $Matches.git
    RevMatch = $revMatch
    Rev = if ($revMatch.Success) { $revMatch.Groups['rev'].Value } else { $null }
  }
}

function Get-GodotRevFromLockfile([string]$LockfilePath, [string]$RepositoryUrl) {
  if (-not (Test-Path -LiteralPath $LockfilePath)) {
    throw "Cargo.lock not found: $LockfilePath"
  }

  $normalizedUrl = $RepositoryUrl -replace '\.git$', ''
  $escapedUrl = [regex]::Escape($normalizedUrl)
  $content = Get-Content -Raw -LiteralPath $LockfilePath
  $pattern = "(?ms)\[\[package\]\]\s*name\s*=\s*`"godot`".*?source\s*=\s*`"git\+$escapedUrl(?:\.git)?(?:\?[^`"#]+)?#(?<rev>[0-9a-f]{40})`""
  $match = [regex]::Match($content, $pattern)
  if (-not $match.Success) {
    throw "Could not find the resolved 'godot' git rev in $LockfilePath"
  }

  return $match.Groups['rev'].Value
}

function Get-GodotRevState([string]$CargoTomlPath, [string]$LockfilePath) {
  $matchInfo = Get-GodotRevMatch -CargoTomlPath $CargoTomlPath
  $lockfileRev = $null

  if (Test-Path -LiteralPath $LockfilePath) {
    $lockfileRev = Get-GodotRevFromLockfile -LockfilePath $LockfilePath -RepositoryUrl $matchInfo.GitUrl
  }

  return @{
    Dependency = $matchInfo
    ManifestRev = $matchInfo.Rev
    LockfileRev = $lockfileRev
    ResolvedRev = if ($matchInfo.Rev) { $matchInfo.Rev } elseif ($lockfileRev) { $lockfileRev } else { $null }
  }
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($Path, $Content, $utf8NoBom)
}

function Set-GodotRev([string]$CargoTomlPath, [string]$NewRev) {
  $matchInfo = Get-GodotRevMatch -CargoTomlPath $CargoTomlPath
  $content = $matchInfo.Content
  $body = $matchInfo.Body

  if ($matchInfo.Rev -eq $NewRev) {
    return $false
  }

  if ($matchInfo.RevMatch.Success) {
    $revGroup = $matchInfo.RevMatch.Groups['rev']
    $bodyUpdated = $body.Substring(0, $revGroup.Index) + $NewRev + $body.Substring($revGroup.Index + $revGroup.Length)
  } else {
    if ($body -match '\b(branch|tag)\s*=') {
      throw "The 'godot' dependency in $CargoTomlPath uses branch/tag selectors without a pinned rev. Pin it manually before running this script."
    }

    $trimmedBody = $body -replace '\s+$', ''
    $trailingWhitespace = $body.Substring($trimmedBody.Length)

    if ([string]::IsNullOrWhiteSpace($trimmedBody)) {
      $bodyUpdated = " rev = `"$NewRev`"" + $trailingWhitespace
    } elseif ($trimmedBody.TrimEnd().EndsWith(',')) {
      $bodyUpdated = $trimmedBody + " rev = `"$NewRev`"" + $trailingWhitespace
    } else {
      $bodyUpdated = $trimmedBody + ", rev = `"$NewRev`"" + $trailingWhitespace
    }
  }

  $updatedMatch = $matchInfo.Match.Groups['prefix'].Value + $bodyUpdated + $matchInfo.Match.Groups['suffix'].Value
  $updated = $content.Substring(0, $matchInfo.Match.Index) + $updatedMatch + $content.Substring($matchInfo.Match.Index + $matchInfo.Match.Length)
  Write-Utf8NoBom -Path $CargoTomlPath -Content $updated
  return $true
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$rustDir = Join-Path $repoRoot "rust"
$cargoTomlPath = Join-Path $rustDir "Cargo.toml"
$lockfilePath = Join-Path $rustDir "Cargo.lock"

if (-not (Test-Path $cargoTomlPath)) {
  throw "Cargo.toml not found: $cargoTomlPath"
}

$revState = Get-GodotRevState -CargoTomlPath $cargoTomlPath -LockfilePath $lockfilePath
$currentRev = if ($revState.LockfileRev) { $revState.LockfileRev } else { $revState.ResolvedRev }
$latestRev = Get-LatestGitRev -RepositoryUrl $RepoUrl -BranchName $Branch

if ($revState.ManifestRev -and $revState.LockfileRev -and $revState.ManifestRev -ne $revState.LockfileRev) {
  Write-Warning "Cargo.toml and Cargo.lock are out of sync for the 'godot' dependency. The lockfile will be refreshed."
}

if ($currentRev) {
  Write-Host "Current gdext rev: $currentRev"
} else {
  Write-Host "Current gdext rev: (unresolved)"
}

Write-Host "Latest  gdext rev: $latestRev"

$needsManifestUpdate = $revState.ManifestRev -ne $latestRev
$needsLockfileUpdate = -not $SkipLockfile -and (($null -eq $revState.LockfileRev) -or $revState.LockfileRev -ne $latestRev)

if (-not $needsManifestUpdate -and -not $needsLockfileUpdate) {
  Write-Host "gdext dependency is already up to date."
  exit 0
}

if ($DryRun) {
  if ($needsManifestUpdate) {
    Write-Host "Dry run: would update rust/Cargo.toml to rev $latestRev"
  }
  if ($needsLockfileUpdate) {
    Write-Host "Dry run: would run 'cargo update -p godot --precise $latestRev'"
  }
  exit 0
}

if (-not $SkipLockfile) {
  Assert-CommandExists "cargo"
}

$originalCargoToml = Get-Content -Raw -LiteralPath $cargoTomlPath
$hadLockfile = Test-Path $lockfilePath
$originalLockfileBytes = $null
if ($hadLockfile) {
  $originalLockfileBytes = [System.IO.File]::ReadAllBytes($lockfilePath)
}

try {
  if ($needsManifestUpdate) {
    Set-GodotRev -CargoTomlPath $cargoTomlPath -NewRev $latestRev | Out-Null
  }

  if ($needsLockfileUpdate) {
    Push-Location $rustDir
    try {
      & cargo update -p godot --precise $latestRev
      if ((Test-Path variable:LASTEXITCODE) -and $LASTEXITCODE -ne 0) {
        throw "cargo update failed."
      }
    } finally {
      Pop-Location
    }
  }
}
catch {
  Write-Utf8NoBom -Path $cargoTomlPath -Content $originalCargoToml
  if ($hadLockfile -and $null -ne $originalLockfileBytes) {
    [System.IO.File]::WriteAllBytes($lockfilePath, $originalLockfileBytes)
  }
  throw
}

Write-Host "Updated gdext dependency to rev $latestRev"

