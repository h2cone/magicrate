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

function Invoke-RustBuild([string]$RustDir, [string]$BuildMode) {
  if ($BuildMode -eq "None") {
    return
  }

  Assert-CommandExists "cargo"
  Write-Host "Building Rust GDExtension ($BuildMode)..."

  Push-Location $RustDir
  try {
    switch ($BuildMode) {
      "Debug" { & cargo build --locked }
      "Release" { & cargo build --release --locked }
      "Both" {
        & cargo build --release --locked
        & cargo build --locked
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

if (-not (Test-Path $projectFile)) {
  throw "Godot project file not found: $projectFile"
}

Invoke-RustBuild -RustDir $rustDir -BuildMode $Build

Assert-CommandExists $GodotExe

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
& $GodotExe @launchArgs

if (Test-Path variable:LASTEXITCODE) {
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}
