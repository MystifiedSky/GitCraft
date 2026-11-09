# One-time setup for automatic updates (run from the app folder):
#   powershell -ExecutionPolicy Bypass -File scripts\setup-updates.ps1
#
# 1. Creates the update signing key pair in your user folder (never inside the project).
# 2. Writes the public key into tauri.conf.json, so installed copies only accept updates you signed.
# 3. Stores the private key and its password as GitHub Actions secrets (needs the GitHub CLI, `gh`),
#    or tells you exactly what to paste into GitHub if `gh` isn't installed.
param(
    [string]$Repo = "MystifiedSky/GitCraft",
    [string]$KeyPath = (Join-Path $env:USERPROFILE ".tauri\gitcraft-updater.key")
)
$ErrorActionPreference = "Stop"
$app = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $app "ui\node_modules\@tauri-apps\cli\tauri.js"
if (-not (Test-Path $cli)) { throw "Run 'npm --prefix ui ci' in $app first." }

$password = ""
if (Test-Path $KeyPath) {
    Write-Host "Using the existing key at $KeyPath"
    $secure = Read-Host "Password of that key (press Enter if it has none)" -AsSecureString
} else {
    New-Item -ItemType Directory -Force (Split-Path $KeyPath) | Out-Null
    $secure = Read-Host "Choose a password for the new signing key" -AsSecureString
}
$password = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure))
if (-not (Test-Path $KeyPath)) {
    & node $cli signer generate --ci -p $password -w $KeyPath
    if ($LASTEXITCODE -ne 0) { throw "Could not create the key." }
}
$pub = (Get-Content "$KeyPath.pub" -Raw).Trim()

# Put the public key in tauri.conf.json (node keeps the file's formatting).
$conf = Join-Path $app "tauri.conf.json"
& node -e "const fs=require('fs');const p=process.argv[1];const c=JSON.parse(fs.readFileSync(p,'utf8'));c.plugins.updater.pubkey=process.argv[2];fs.writeFileSync(p,JSON.stringify(c,null,2)+'\n')" $conf $pub
Write-Host "Public key written to tauri.conf.json. Commit that change."

$gh = Get-Command gh -ErrorAction SilentlyContinue
if ($gh) {
    Get-Content $KeyPath -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $Repo
    $password | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $Repo
    Write-Host "Secrets TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD saved on $Repo."
} else {
    Write-Host ""
    Write-Host "GitHub CLI not found. On https://github.com/$Repo/settings/secrets/actions add two secrets:"
    Write-Host "  TAURI_SIGNING_PRIVATE_KEY           = the whole contents of $KeyPath"
    Write-Host "  TAURI_SIGNING_PRIVATE_KEY_PASSWORD  = the password you just typed"
}
Write-Host ""
Write-Host "IMPORTANT: back up $KeyPath and its password somewhere safe (a password manager)."
Write-Host "If they are lost, copies already installed can never be updated automatically again."
