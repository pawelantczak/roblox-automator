param (
    [string]$BumpType = "patch"
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

$VersionFile = Join-Path $ScriptDir "VERSION"
if (-not (Test-Path $VersionFile)) {
    Set-Content -Path $VersionFile -Value "0.1.0"
}

$CurrentVersion = (Get-Content $VersionFile).Trim()
$Parts = $CurrentVersion.Split('.')
$Major = [int]$Parts[0]
$Minor = [int]$Parts[1]
$Patch = [int]$Parts[2]

switch -Regex ($BumpType) {
    "^patch$" {
        $Patch++
        $NewVersion = "$Major.$Minor.$Patch"
    }
    "^minor$" {
        $Minor++
        $Patch = 0
        $NewVersion = "$Major.$Minor.$Patch"
    }
    "^major$" {
        $Major++
        $Minor = 0
        $Patch = 0
        $NewVersion = "$Major.$Minor.$Patch"
    }
    "^\d+\.\d+\.\d+$" {
        $NewVersion = $BumpType
    }
    default {
        Write-Error "Usage: .\bump.ps1 [patch | minor | major | <version>]"
        exit 1
    }
}

Write-Host "Bumping version: $CurrentVersion -> $NewVersion" -ForegroundColor Cyan
Set-Content -Path $VersionFile -Value $NewVersion

# 1. Synchronize Cargo.toml version
$CargoToml = Join-Path $ScriptDir "Cargo.toml"
if (Test-Path $CargoToml) {
    $CargoContent = Get-Content $CargoToml -Raw
    $CargoContent = ([regex]'(?m)^version\s*=\s*"[^"]+"').Replace($CargoContent, "version = `"$NewVersion`"", 1)
    Set-Content -Path $CargoToml -Value $CargoContent
    Write-Host "==> Updated Cargo.toml to version $NewVersion" -ForegroundColor Green
}

# 2. Synchronize CHANGELOG.md & RELEASE_NOTES.md
$Today = (Get-Date).ToString("yyyy-MM-dd")
foreach ($DocName in @("CHANGELOG.md", "RELEASE_NOTES.md")) {
    $DocFile = Join-Path $ScriptDir $DocName
    if (Test-Path $DocFile) {
        $DocContent = Get-Content $DocFile -Raw
        if ($DocContent -notmatch "## \[$NewVersion\]") {
            $Pattern = '(?s)## \[Unreleased\]\s*\n(.*?)(?=\n---\s*\n## \[|\n## \[|\Z)'
            $Match = ([regex]$Pattern).Match($DocContent)
            if ($Match.Success) {
                $Body = $Match.Groups[1].Value.Trim()
                if ($Body) {
                    $Replacement = "## [Unreleased]`n`n---`n`n## [$NewVersion] - $Today`n`n$Body`n"
                    $DocContent = $DocContent.Substring(0, $Match.Index) + $Replacement + $DocContent.Substring($Match.Index + $Match.Length)
                    Set-Content -Path $DocFile -Value $DocContent
                    Write-Host "==> Updated $DocName with release header for v$NewVersion" -ForegroundColor Green
                }
            }
        }
    }
}

# 3. Verify Build
Write-Host "==> Verifying cargo build..." -ForegroundColor Cyan
cargo build --release

Write-Host ""
Write-Host "==========================================" -ForegroundColor Green
Write-Host "Version bumped to v$NewVersion successfully!" -ForegroundColor Green
Write-Host "To publish this release to GitHub, run:" -ForegroundColor Yellow
Write-Host "  git commit -am `"chore: release v$NewVersion`""
Write-Host "  git tag v$NewVersion"
Write-Host "  git push origin main --tags"
Write-Host "==========================================" -ForegroundColor Green
