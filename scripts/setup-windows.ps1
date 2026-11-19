<#
.SYNOPSIS
    Prepares Windows to build an xtool-based iOS project in WSL and install
    it on an iPhone, then starts pippwin's guided setup inside WSL.

.DESCRIPTION
    1. Checks it runs as administrator, or starts itself again as
       administrator (Windows asks for permission).
    2. Installs WSL with Ubuntu 24.04 if it is missing. When Windows needs a
       restart, it says so: restart, then run this script again.
    3. Installs usbipd-win with winget if it is missing (it passes the
       iPhone's USB connection through to WSL).
    4. Inside Ubuntu, clones https://github.com/buchochelliq-labs/pippwin
       into ~/.local/src/pippwin (or updates that clone), then runs its
       scripts/bootstrap.sh. That installs git, curl and a C compiler with
       apt and rustup (verified), builds pippwin with
       `cargo build --release --locked`, installs it to
       ~/.local/bin/pippwin and starts `pippwin setup` with -Project and
       -Branch.

    Nothing is downloaded and run from PowerShell: no Invoke-Expression of
    remote content. The execution policy is not changed (run it with
    `powershell -ExecutionPolicy Bypass -File setup-windows.ps1`, which
    applies to that one process only).

.PARAMETER Project
    The project to set up: its https git URL. Asked for inside WSL when
    left out.

.PARAMETER Branch
    The project's branch. Asked for inside WSL when left out.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\setup-windows.ps1

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\setup-windows.ps1 -Project https://github.com/owner/app
#>
[CmdletBinding()]
param(
    [string]$Project = '',
    [string]$Branch = '',
    [string]$Distro = 'Ubuntu-24.04',
    # pippwin's own branch or tag to build.
    [string]$PippwinRef = 'main'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$PippwinUrl = 'https://github.com/buchochelliq-labs/pippwin'

function Write-Step([string]$Message) {
    Write-Host "==> $Message" -ForegroundColor Cyan
}

# Values passed into WSL's shell: plain characters only, never option-like.
foreach ($value in @($Distro, $Branch, $PippwinRef)) {
    if ($value -and ($value -notmatch '^[A-Za-z0-9._/-]+$' -or $value.StartsWith('-') -or $value.Contains('..'))) {
        throw "Not a plain name: '$value'"
    }
}
if ($Project -and ($Project -notmatch '^[A-Za-z0-9._/:~+-]+$' -or $Project.StartsWith('-'))) {
    throw "Not a git URL or a WSL folder: '$Project'"
}

# 1. Administrator rights (for WSL and usbipd-win).
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Step 'Administrator rights are needed to install WSL and usbipd-win; Windows will ask for permission.'
    $arguments = @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"",
        '-Distro', $Distro, '-PippwinRef', $PippwinRef
    )
    if ($Project) { $arguments += @('-Project', $Project) }
    if ($Branch) { $arguments += @('-Branch', $Branch) }
    Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList $arguments
    exit 0
}

# wsl.exe prints UTF-16 unless told otherwise.
$env:WSL_UTF8 = '1'

# 2. WSL with Ubuntu.
$installed = @()
if (Get-Command wsl.exe -ErrorAction SilentlyContinue) {
    $list = & wsl.exe --list --quiet 2>$null
    if ($LASTEXITCODE -eq 0 -and $list) {
        $installed = @($list | ForEach-Object { $_.Trim([char]0, ' ') } | Where-Object { $_ })
    }
}
if ($installed -notcontains $Distro) {
    Write-Step "Installing WSL with $Distro"
    & wsl.exe --install --distribution $Distro
    if ($LASTEXITCODE -ne 0) {
        throw "wsl --install failed (exit code $LASTEXITCODE)."
    }
    Write-Host ''
    Write-Host "If Windows asked you to restart, restart now. Then open '$Distro' from the Start menu once,"
    Write-Host 'create your Linux user name and password when it asks, and run this script again.'
    exit 0
}

$linuxUser = [string](& wsl.exe -d $Distro -- whoami)
if ($LASTEXITCODE -ne 0 -or -not $linuxUser.Trim() -or $linuxUser.Trim() -eq 'root') {
    Write-Host "Open '$Distro' from the Start menu once and create your Linux user, then run this script again."
    exit 1
}

# 3. usbipd-win.
if (-not (Get-Command usbipd -ErrorAction SilentlyContinue)) {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw 'winget is missing. Install "App Installer" from the Microsoft Store, or usbipd-win from https://github.com/dorssel/usbipd-win/releases, then run this script again.'
    }
    Write-Step 'Installing usbipd-win'
    & winget install --interactive --exact dorssel.usbipd-win
    if ($LASTEXITCODE -ne 0) {
        throw "winget could not install usbipd-win (exit code $LASTEXITCODE)."
    }
    # Pick up its PATH entry, and restart the distribution so WSL sees it.
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
        [Environment]::GetEnvironmentVariable('Path', 'User')
    & wsl.exe --terminate $Distro | Out-Null
}

# 4. Inside Ubuntu: git, pippwin's clone, then bootstrap.sh (build,
#    install, start).
Write-Step "Preparing $Distro (sudo may ask for your Linux password)"
& wsl.exe -d $Distro --cd '~' --exec bash -c 'command -v git >/dev/null || { sudo apt-get update && sudo apt-get install -y git ca-certificates; }'
if ($LASTEXITCODE -ne 0) { throw 'Installing git in WSL failed.' }

# --exec runs bash directly. With `--` instead, wsl.exe hands the whole
# line to the default Linux shell first, which expands $1, $2, $d and
# $HOME before this bash sees them, so the clone got no branch or URL.
# No double quotes inside: Windows PowerShell mangles them on the way to
# native programs. The values were checked to be plain above, and reach
# bash as separate arguments ($1, $2), never as script text.
Write-Step "Getting pippwin ($PippwinRef) inside $Distro"
& wsl.exe -d $Distro --cd '~' --exec bash -c 'set -e; d=$HOME/.local/src/pippwin; if test -e $d/.git; then git -C $d remote get-url origin | grep -qxF -e $2 -e $2.git || { echo $d is a clone of another repository >&2; exit 3; }; if git -C $d status --porcelain | grep -q .; then echo $d has local changes >&2; exit 4; fi; git -C $d fetch origin -- $1; git -C $d checkout --quiet --detach FETCH_HEAD; else mkdir -p $HOME/.local/src; git clone --branch $1 -- $2 $d; fi' _ $PippwinRef $PippwinUrl
if ($LASTEXITCODE -ne 0) { throw 'Getting pippwin in WSL failed.' }

Write-Step 'Building and starting pippwin'
$bootstrap = @('.local/src/pippwin/scripts/bootstrap.sh', '--pippwin-ref', $PippwinRef)
if ($Project) { $bootstrap += @('--project', $Project) }
if ($Branch) { $bootstrap += @('--branch', $Branch) }
& wsl.exe -d $Distro --cd '~' --exec bash @bootstrap
exit $LASTEXITCODE
