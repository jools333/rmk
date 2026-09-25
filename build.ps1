# PowerShell script to build all Charybdis Mini RMK UF2 files via WSL
Write-Host "Running RMK build in WSL..." -ForegroundColor Cyan
wsl -d Ubuntu -u root --cd /root/projects/rmk/charybdis-rmk -e bash -c "./build.sh"
if ($LASTEXITCODE -eq 0) {
    Write-Host "`nAll UF2 binaries are ready in: Z:\rmk\charybdis-rmk\dist\" -ForegroundColor Green
} else {
    Write-Host "`nBuild failed with exit code $LASTEXITCODE" -ForegroundColor Red
}
