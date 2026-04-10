param(
  [ValidateSet("Debug", "Release", "Both", "None")]
  [string]$Build = "Debug",

  [string]$GodotExe = "godot",
  [switch]$Editor,
  [switch]$Headless,

  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$GodotArgs
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-CommandExists([string]$CommandName) {
  $cmd = Get-Command $CommandName -ErrorAction SilentlyContinue
  if (-not $cmd) {
    throw "Command not found: '$CommandName'. Ensure it is installed and on PATH."
  }
}

function Assert-LastExitCode([string]$Action) {
  if ((Test-Path variable:LASTEXITCODE) -and $LASTEXITCODE -ne 0) {
    throw "$Action failed with exit code $LASTEXITCODE."
  }
}

function Resolve-CommandPath([string]$CommandName) {
  $cmd = Get-Command $CommandName -ErrorAction SilentlyContinue
  if (-not $cmd) {
    throw "Command not found: '$CommandName'. Ensure it is installed and on PATH."
  }

  if ($cmd.Path) {
    return $cmd.Path
  }

  return $cmd.Source
}

function Test-IsWindows() {
  return [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
}

function Resolve-GodotExecutable([string]$RequestedExe) {
  $resolvedExe = Resolve-CommandPath $RequestedExe
  if (-not (Test-IsWindows)) {
    return $resolvedExe
  }

  $dir = Split-Path -Parent $resolvedExe
  $base = [System.IO.Path]::GetFileNameWithoutExtension($resolvedExe)
  $ext = [System.IO.Path]::GetExtension($resolvedExe)
  if ($base -match '(?i)_console$') {
    return $resolvedExe
  }

  $consoleSibling = Join-Path $dir ($base + "_console" + $ext)
  if (Test-Path -LiteralPath $consoleSibling) {
    return $consoleSibling
  }

  return $resolvedExe
}

function Write-Utf8NoBom([string]$Path, [string]$Value) {
  $parent = Split-Path -Parent $Path
  if ($parent -and -not (Test-Path -LiteralPath $parent)) {
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
  }

  $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($Path, $Value, $utf8NoBom)
}

function Normalize-GodotExtensionList([string]$GodotProjectDir) {
  $godotCacheDir = Join-Path $GodotProjectDir ".godot"
  $extensionListPath = Join-Path $godotCacheDir "extension_list.cfg"
  $defaultExt = "res://rust.gdextension"

  if (-not (Test-Path -LiteralPath $godotCacheDir)) {
    New-Item -ItemType Directory -Force -Path $godotCacheDir | Out-Null
  }

  $kept = [System.Collections.Generic.List[string]]::new()
  if (Test-Path -LiteralPath $extensionListPath) {
    $raw = Get-Content -Raw -LiteralPath $extensionListPath
    $lines = $raw -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_.Length -gt 0 }

    foreach ($line in $lines) {
      if ($line -notmatch '^res://') { continue }
      $rel = $line.Substring(6)
      $fsPath = Join-Path $GodotProjectDir $rel
      if (Test-Path -LiteralPath $fsPath) {
        [void]$kept.Add($line)
      }
    }
  }

  $defaultRel = $defaultExt.Substring(6)
  if (Test-Path -LiteralPath (Join-Path $GodotProjectDir $defaultRel)) {
    if (-not ($kept -contains $defaultExt)) {
      $kept.Insert(0, $defaultExt)
    }
  }

  Write-Utf8NoBom -Path $extensionListPath -Value ($kept -join "`n")
}

function Test-GodotImportNeeded([string]$GodotProjectDir) {
  $importedDir = Join-Path $GodotProjectDir ".godot/imported"
  if (-not (Test-Path -LiteralPath $importedDir)) {
    return $true
  }

  return -not (Get-ChildItem -LiteralPath $importedDir -File -ErrorAction SilentlyContinue | Select-Object -First 1)
}

function Ensure-GodotImported([string]$GodotExePath, [string]$GodotProjectDir) {
  Normalize-GodotExtensionList -GodotProjectDir $GodotProjectDir

  if (-not (Test-GodotImportNeeded -GodotProjectDir $GodotProjectDir)) {
    return
  }

  Write-Host "Importing Godot assets for first run..."
  & $GodotExePath --path $GodotProjectDir --import --quit
  Assert-LastExitCode "$GodotExePath --path $GodotProjectDir --import --quit"

  Normalize-GodotExtensionList -GodotProjectDir $GodotProjectDir
}

function Invoke-RustBuild([string]$RustDir, [string]$BuildMode) {
  if ($BuildMode -eq "None") {
    return
  }

  Assert-CommandExists "cargo"
  Write-Host "Building Rust GDExtension ($BuildMode)..."

  Push-Location $RustDir
  try {
    switch ($BuildMode) {
      "Debug" {
        & cargo build --locked
        Assert-LastExitCode "cargo build --locked"
      }
      "Release" {
        & cargo build --release --locked
        Assert-LastExitCode "cargo build --release --locked"
      }
      "Both" {
        & cargo build --release --locked
        Assert-LastExitCode "cargo build --release --locked"
        & cargo build --locked
        Assert-LastExitCode "cargo build --locked"
      }
    }
  } finally {
    Pop-Location
  }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$rustDir = Join-Path $repoRoot "rust"
$godotDir = Join-Path $repoRoot "godot"
$projectFile = Join-Path $godotDir "project.godot"

if (-not (Test-Path -LiteralPath $projectFile)) {
  throw "Godot project file not found: $projectFile"
}

Invoke-RustBuild -RustDir $rustDir -BuildMode $Build

Assert-CommandExists $GodotExe
$resolvedGodotExe = Resolve-GodotExecutable $GodotExe
Write-Host "Using Godot executable: $resolvedGodotExe"

Ensure-GodotImported -GodotExePath $resolvedGodotExe -GodotProjectDir $godotDir

$launchArgs = @()
if ($Headless) {
  $launchArgs += "--headless"
}
if ($Editor) {
  $launchArgs += "-e"
}
$launchArgs += @("--path", $godotDir)
if ($GodotArgs) {
  $launchArgs += $GodotArgs
}

Write-Host "Launching Godot..."
& $resolvedGodotExe @launchArgs
Assert-LastExitCode "$resolvedGodotExe $($launchArgs -join ' ')"
