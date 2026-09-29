# ==============================================================================
# flash.ps1 — Автоматическая поочередная прошивка устройств Charybdis Mini (RMK)
# ==============================================================================
# Поддерживает поочередную заливку всех узлов (левая -> правая -> донгл)
# или прошивку отдельных устройств по выбору.
#
# Использование:
#   .\flash.ps1          # Залить все по очереди: Left -> Right -> Dongle
#   .\flash.ps1 left     # Только левая половинка (Peripheral)
#   .\flash.ps1 right    # Только правая половинка (Central)
#   .\flash.ps1 dongle   # Только USB донгл
#   .\flash.ps1 reset    # Залить сброс настроек (settings_reset)
# ==============================================================================

param (
    [string]$Target = "all"
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$distDir = Join-Path $scriptDir "dist"

# Бинарники
$fileLeft    = Join-Path $distDir "charybdis_left_peripheral.uf2"
$fileRight   = Join-Path $distDir "charybdis_right_central.uf2"
$fileDongle  = Join-Path $distDir "charybdis_dongle.uf2"
$fileReset   = Join-Path $distDir "charybdis_settings_reset.uf2"

# Проверка наличия бинарников
function Check-Binaries {
    $needed = @($fileLeft, $fileRight, $fileDongle)
    foreach ($f in $needed) {
        if (-not (Test-Path $f)) {
            Write-Host "[-] Ошибка: Файл $f не найден!" -ForegroundColor Red
            Write-Host "    Сначала скомпилируйте прошивку через .\build.ps1" -ForegroundColor Yellow
            exit 1
        }
    }
}

# Поиск смонтированного диска загрузчика UF2
function Get-Uf2Drive {
    $volumes = Get-Volume -ErrorAction SilentlyContinue | Where-Object { 
        $_.DriveType -eq 'Removable' -or $_.FileSystemLabel -match 'NICENANO|NRF52|MICRO|BOOT|ADAFRUIT' 
    }

    foreach ($v in $volumes) {
        if ($v.DriveLetter) {
            $root = "$($v.DriveLetter):\"
            $infoPath = Join-Path $root "INFO_UF2.TXT"
            if (Test-Path $infoPath) {
                return $root
            }
            # Проверка по метке
            if ($v.FileSystemLabel -match 'NICENANO|NRF52|MICRO|BOOT') {
                return $root
            }
        }
    }
    return $null
}

# Ожидание отключения предыдущего накопителя
function Wait-ForDisconnect {
    $drive = Get-Uf2Drive
    if ($drive) {
        Write-Host "[!] Ожидание отключения/перезагрузки предыдущего устройства..." -ForegroundColor Yellow
        while (Get-Uf2Drive) {
            Start-Sleep -Milliseconds 500
        }
        Write-Host "[✓] Устройство отключено/перезагружено." -ForegroundColor Green
        Start-Sleep -Seconds 1
    }
}

# Ожидание подключения целевого устройства
function Wait-ForDevice {
    param ([string]$DeviceName)

    Write-Host ""
    Write-Host "======================================================================" -ForegroundColor Cyan
    Write-Host " ОЖИДАНИЕ УСТРОЙСТВА: $DeviceName" -ForegroundColor Yellow
    Write-Host "======================================================================" -ForegroundColor Cyan
    Write-Host " 👉 Подключите $DeviceName по USB к компьютеру."
    Write-Host " 👉 Дважды нажмите кнопку Reset на контроллере (двойной клик),"
    Write-Host "    чтобы перевести его в режим загрузчика (диск NICENANO)."
    Write-Host "    (Для отмены нажмите Ctrl+C)"
    Write-Host ""

    $spin = @('|', '/', '-', '\')
    $idx = 0

    while ($true) {
        $drive = Get-Uf2Drive
        if ($drive) {
            Write-Host "`r[✓] Обнаружен диск загрузчика: $drive                     " -ForegroundColor Green
            try { [Console]::Beep(1000, 200) } catch {}
            return $drive
        }

        $char = $spin[$idx % 4]
        Write-Host -NoNewline "`r[*] Ожидание режима загрузчика (двойной клик Reset)... $char "
        Start-Sleep -Milliseconds 300
        $idx++
    }
}

# Прошивка одного устройства
function Flash-Device {
    param (
        [string]$DeviceName,
        [string]$Uf2File
    )

    Wait-ForDisconnect
    $drive = Wait-ForDevice -DeviceName $DeviceName

    Write-Host "[*] Прошивка: $DeviceName ..." -ForegroundColor Cyan
    Write-Host "    Файл: $(Split-Path $Uf2File -Leaf)" -ForegroundColor Gray
    Write-Host "    Назначение: $drive" -ForegroundColor Gray

    try {
        Copy-Item -Path $Uf2File -Destination $drive -Force
        Write-Host "[✓] Файл успешно скопирован! Контроллер перезагружается..." -ForegroundColor Green
        try { 
            [Console]::Beep(1200, 150)
            [Console]::Beep(1600, 250)
        } catch {}
    } catch {
        Write-Host "[-] Ошибка копирования файла: $_" -ForegroundColor Red
        return $false
    }

    # Даем контроллеру 2 секунды на запись во флеш и перезагрузку
    Start-Sleep -Seconds 2
    return $true
}

# Точка входа
Check-Binaries

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "   Charybdis Mini RMK — Мастер автоматической прошивки (Windows)      " -ForegroundColor White
Write-Host "======================================================================" -ForegroundColor Cyan

switch ($Target.ToLower()) {
    "left" {
        Flash-Device -DeviceName "ЛЕВАЯ половинка (Peripheral)" -Uf2File $fileLeft
    }
    "right" {
        Flash-Device -DeviceName "ПРАВАЯ половинка (Central)" -Uf2File $fileRight
    }
    "dongle" {
        Flash-Device -DeviceName "USB Донгл" -Uf2File $fileDongle
    }
    "reset" {
        Flash-Device -DeviceName "Сброс настроек Flash (settings_reset)" -Uf2File $fileReset
    }
    default {
        # Все устройства по очереди
        Write-Host "Будут по очереди прошиты 3 устройства:" -ForegroundColor Yellow
        Write-Host "  1. Левая половинка (Peripheral)"
        Write-Host "  2. Правая половинка (Central)"
        Write-Host "  3. USB Донгл"
        Write-Host ""

        # 1. Левая половинка
        $ok = Flash-Device -DeviceName "1. ЛЕВАЯ половинка (Peripheral)" -Uf2File $fileLeft
        if (-not $ok) { exit 1 }

        # 2. Правая половинка
        $ok = Flash-Device -DeviceName "2. ПРАВАЯ половинка (Central)" -Uf2File $fileRight
        if (-not $ok) { exit 1 }

        # 3. Донгл
        $ok = Flash-Device -DeviceName "3. USB Донгл" -Uf2File $fileDongle
        if (-not $ok) { exit 1 }

        Write-Host ""
        Write-Host "======================================================================" -ForegroundColor Green
        Write-Host "   [✓] ВСЕ УСТРОЙСТВА УСПЕШНО ПРОШИТЫ!                                " -ForegroundColor Green
        Write-Host "======================================================================" -ForegroundColor Green
        try {
            [Console]::Beep(1000, 100)
            [Console]::Beep(1200, 100)
            [Console]::Beep(1500, 300)
        } catch {}
    }
}
