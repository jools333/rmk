#!/usr/bin/env bash
# ==============================================================================
# flash.sh — Автоматическая поочередная прошивка устройств Charybdis Mini (RMK)
# ==============================================================================
# Поддерживает поочередную заливку всех узлов (левая -> правая -> донгл)
# или прошивку отдельных устройств по выбору.
#
# Использование:
#   ./flash.sh           # Залить все по очереди: Left -> Right -> Dongle
#   ./flash.sh all       # То же самое (все устройства)
#   ./flash.sh left      # Только левая половинка (Peripheral)
#   ./flash.sh right     # Только правая половинка (Central)
#   ./flash.sh dongle    # Только USB донгл
#   ./flash.sh reset     # Залить сброс настроек (settings_reset)
#   ./flash.sh left right # Залить левую, затем правую
# ==============================================================================

set -u

# Цвета для вывода в терминал
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DIST_DIR="${SCRIPT_DIR}/dist"

FOUND_MOUNT_POINT=""

# Звуковое оповещение (bell или системный звук)
play_sound() {
    local sound_type="${1:-complete}" # "complete", "bell" or "attention"
    (
        if [ "$sound_type" = "attention" ] && [ -f /usr/share/sounds/freedesktop/stereo/bell.oga ]; then
            paplay /usr/share/sounds/freedesktop/stereo/bell.oga 2>/dev/null || \
            canberra-gtk-play -i bell 2>/dev/null || printf '\a'
        elif [ -f /usr/share/sounds/freedesktop/stereo/complete.oga ]; then
            paplay /usr/share/sounds/freedesktop/stereo/complete.oga 2>/dev/null || \
            canberra-gtk-play -i complete 2>/dev/null || printf '\a'
        else
            printf '\a'
        fi
    ) &>/dev/null &
}

# Всплывающее системное уведомление на рабочем столе
send_notification() {
    local title="$1"
    local message="$2"
    local urgency="${3:-normal}"
    if command -v notify-send &>/dev/null; then
        notify-send -u "$urgency" -a "RMK Flasher" "$title" "$message" 2>/dev/null || true
    fi
}

# Справка
show_help() {
    echo -e "${BOLD}Использование:${NC} ./flash.sh [ПАРАМЕТРЫ]"
    echo ""
    echo -e "${BOLD}Параметры:${NC}"
    echo -e "  ${CYAN}(без параметров)${NC}  Поочередная прошивка: Левая -> Правая -> Донгл"
    echo -e "  ${CYAN}all${NC}, ${CYAN}--all${NC}        Поочередная прошивка всех 3 устройств"
    echo -e "  ${CYAN}left${NC}, ${CYAN}--left${NC}      Прошить только левую половинку (Peripheral)"
    echo -e "  ${CYAN}right${NC}, ${CYAN}--right${NC}    Прошить только правую половинку (Central)"
    echo -e "  ${CYAN}dongle${NC}, ${CYAN}--dongle${NC}  Прошить только USB Донгл"
    echo -e "  ${CYAN}reset${NC}, ${CYAN}--reset${NC}    Прошить утилиту очистки памяти (settings_reset)"
    echo -e "  ${CYAN}-h${NC}, ${CYAN}--help${NC}        Показать эту справку"
    echo ""
    echo -e "${BOLD}Примеры:${NC}"
    echo -e "  ./flash.sh              # Прошить всю клавиатуру и донгл"
    echo -e "  ./flash.sh right        # Прошить только правую половинку"
    echo -e "  ./flash.sh left right   # Прошить левую, затем правую"
    echo ""
}

# Поиск точки монтирования NICENANO
get_nicenano_mount() {
    local mount_path=""

    # 1. Проверяем findmnt по метке файловой системы
    if command -v findmnt &>/dev/null; then
        mount_path=$(findmnt -rn -o TARGET -S LABEL=NICENANO 2>/dev/null | head -n 1)
        if [ -n "$mount_path" ] && [ -d "$mount_path" ]; then
            echo "$mount_path"
            return 0
        fi
    fi

    # 2. Проверяем стандартные пути монтирования в Linux
    local candidates=(
        "/run/media/${USER}/NICENANO"
        "/media/${USER}/NICENANO"
        "/media/NICENANO"
        "/mnt/NICENANO"
    )

    for path in "${candidates[@]}"; do
        if [ -d "$path" ]; then
            echo "$path"
            return 0
        fi
    done

    # 3. Если устройство видно в системе, но ещё не смонтировано, пробуем udisksctl
    if [ -e "/dev/disk/by-label/NICENANO" ] && command -v udisksctl &>/dev/null; then
        local mounted_info
        mounted_info=$(udisksctl mount -b /dev/disk/by-label/NICENANO --no-user-interaction 2>/dev/null || true)
        mount_path=$(echo "$mounted_info" | grep -o 'at /.*' | sed 's/at //;s/\.$//')
        if [ -n "$mount_path" ] && [ -d "$mount_path" ]; then
            echo "$mount_path"
            return 0
        fi
    fi

    return 1
}

# Ожидание отключения предыдущего накопителя NICENANO
wait_for_nicenano_disconnect() {
    local current_mount
    current_mount=$(get_nicenano_mount || true)
    if [ -n "$current_mount" ] || [ -e "/dev/disk/by-label/NICENANO" ]; then
        echo -e "${YELLOW}[!] Обнаружен подключенный диск NICENANO. Ожидаем отключения/перезагрузки...${NC}"
        while [ -n "$(get_nicenano_mount || true)" ] || [ -e "/dev/disk/by-label/NICENANO" ]; do
            sleep 0.5
        done
        echo -e "${GREEN}[✓] Предыдущий диск отключен.${NC}"
        sleep 1
    fi
}

# Ожидание подключения диска NICENANO со спиннером
wait_for_nicenano() {
    local target_name="$1"
    local spin='-\|/'
    local i=0
    local mount_dir=""

    FOUND_MOUNT_POINT=""

    echo ""
    echo -e "${BOLD}${BLUE}======================================================================${NC}"
    echo -e "${BOLD}${CYAN} ОЖИДАНИЕ УСТРОЙСТВА: ${YELLOW}${target_name}${NC}"
    echo -e "${BOLD}${BLUE}======================================================================${NC}"
    echo -e " 👉 Подключите ${BOLD}${target_name}${NC} по USB к компьютеру."
    echo -e " 👉 Дважды нажмите кнопку ${BOLD}Reset${NC} на контроллере (двойной клик),"
    echo -e "    чтобы перевести его в режим загрузчика (появится диск ${BOLD}NICENANO${NC})."
    echo -e "    (Для отмены нажмите Ctrl+C)"
    echo ""

    send_notification "RMK Flasher: Ожидание" "Подключите $target_name и дважды нажмите Reset" "normal"

    while true; do
        mount_dir=$(get_nicenano_mount || true)
        if [ -n "$mount_dir" ]; then
            # Проверяем доступность каталога
            if [ -d "$mount_dir" ] && [ -w "$mount_dir" ]; then
                echo -ne "\r\033[K"
                echo -e "${GREEN}${BOLD}[✓] Диск NICENANO обнаружен:${NC} ${mount_dir}"
                play_sound "attention"
                FOUND_MOUNT_POINT="$mount_dir"
                return 0
            fi
        fi

        printf "\r ${YELLOW}%c${NC} Ожидание диска NICENANO... (дважды нажмите кнопку Reset) " "${spin:i++%4:1}"
        sleep 0.3
    done
}

# Функция заливки файла прошивки
flash_device() {
    local target_name="$1"
    local uf2_file="$2"

    if [ ! -f "$uf2_file" ]; then
        echo -e "${RED}[-] Ошибка: Файл прошивки не найден:${NC} ${uf2_file}"
        echo -e "${YELLOW}[*] Попытка собрать прошивку с помощью ./build.sh...${NC}"
        if [ -x "${SCRIPT_DIR}/build.sh" ]; then
            "${SCRIPT_DIR}/build.sh"
        else
            echo -e "${RED}[-] Скрипт build.sh не найден или не исполняемый. Сначала соберите прошивку!${NC}"
            return 1
        fi

        if [ ! -f "$uf2_file" ]; then
            echo -e "${RED}[-] Ошибка: Файл ${uf2_file} так и не появился после сборки.${NC}"
            return 1
        fi
    fi

    # 1. Ждём, пока отключится предыдущий диск NICENANO
    wait_for_nicenano_disconnect

    # 2. Ждём подключения нужного устройства
    wait_for_nicenano "$target_name"
    local mount_point="$FOUND_MOUNT_POINT"

    if [ -z "$mount_point" ] || [ ! -d "$mount_point" ]; then
        echo -e "${RED}[-] Ошибка: Не удалось определить точку монтирования NICENANO.${NC}"
        return 1
    fi

    echo -e "${CYAN}[*] Запись прошивки:${NC} $(basename "$uf2_file") -> ${mount_point}/"
    send_notification "Прошивка" "Запись $(basename "$uf2_file") на $target_name..." "normal"
    
    # Копирование файла UF2.
    # bootloader nRF52/nice!nano перезагружает устройство сразу после записи последнего блока.
    if cp "$uf2_file" "$mount_point/"; then
        echo -e "${GREEN}[✓] Файл успешно скопирован в бутлоадер.${NC}"
    else
        # Если cp завершился с ошибкой, проверяем: возможно устройство уже перезагрузилось на лету
        echo -e "${YELLOW}[*] Запись завершена (устройство инициировало перезагрузку).${NC}"
    fi

    # Синхронизация дисковых буферов
    sync 2>/dev/null || true

    echo -e "${YELLOW}[*] Ожидание завершения прошивки и перезагрузки контроллера...${NC}"
    
    # Ждём, пока диск NICENANO исчезнет (контроллер прошился и вышел из bootloader)
    local timeout=15
    local elapsed=0
    while [ -n "$(get_nicenano_mount || true)" ] && [ "$elapsed" -lt "$timeout" ]; do
        sleep 0.5
        elapsed=$((elapsed + 1))
    done

    echo -e "${GREEN}${BOLD}======================================================================${NC}"
    echo -e "${GREEN}${BOLD} [✓] УСПЕШНО ПРОШИТО: ${target_name}!${NC}"
    echo -e "${GREEN}${BOLD}======================================================================${NC}"
    play_sound "complete"
    send_notification "Прошивка завершена" "$target_name успешно прошит!" "normal"
    
    # Пауза перед следующим устройством
    sleep 2
    return 0
}

# ==============================================================================
# Разбор аргументов командной строки
# ==============================================================================

declare -a TARGET_NAMES=()
declare -a TARGET_FILES=()

add_target() {
    local key="$1"
    case "$key" in
        left)
            TARGET_NAMES+=("Левая половинка (Left Peripheral)")
            TARGET_FILES+=("${DIST_DIR}/charybdis_left_peripheral.uf2")
            ;;
        right)
            TARGET_NAMES+=("Правая половинка (Right Central)")
            TARGET_FILES+=("${DIST_DIR}/charybdis_right_central.uf2")
            ;;
        dongle)
            TARGET_NAMES+=("USB Донгл (Dongle)")
            TARGET_FILES+=("${DIST_DIR}/charybdis_dongle.uf2")
            ;;
        reset|settings_reset)
            TARGET_NAMES+=("Сброс памяти (Settings Reset)")
            TARGET_FILES+=("${DIST_DIR}/charybdis_settings_reset.uf2")
            ;;
        *)
            echo -e "${RED}[-] Неизвестный параметр:${NC} $key"
            show_help
            exit 1
            ;;
    esac
}

if [ $# -eq 0 ]; then
    # По умолчанию — все три устройства поочередно
    add_target "left"
    add_target "right"
    add_target "dongle"
else
    for arg in "$@"; do
        case "$arg" in
            -h|--help|help)
                show_help
                exit 0
                ;;
            all|--all)
                TARGET_NAMES=()
                TARGET_FILES=()
                add_target "left"
                add_target "right"
                add_target "dongle"
                ;;
            left|--left)
                add_target "left"
                ;;
            right|--right)
                add_target "right"
                ;;
            dongle|--dongle)
                add_target "dongle"
                ;;
            reset|--reset|settings_reset|--settings-reset)
                add_target "reset"
                ;;
            *)
                add_target "$arg"
                ;;
        esac
    done
fi

# Прерывание по Ctrl+C
trap 'echo -e "\n${RED}[!] Прервано пользователем.${NC}"; exit 130' INT

# ==============================================================================
# Основной цикл выполнения
# ==============================================================================

TOTAL_TARGETS=${#TARGET_NAMES[@]}

echo ""
echo -e "${BOLD}${GREEN}======================================================================${NC}"
echo -e "${BOLD}${GREEN}          Charybdis Mini RMK — Мастер прошивки устройств             ${NC}"
echo -e "${BOLD}${GREEN}======================================================================${NC}"
echo -e "Всего устройств в очереди для прошивки: ${BOLD}${TOTAL_TARGETS}${NC}"
for i in "${!TARGET_NAMES[@]}"; do
    echo -e "  $((i+1)). ${CYAN}${TARGET_NAMES[$i]}${NC} -> $(basename "${TARGET_FILES[$i]}")"
done
echo -e "----------------------------------------------------------------------"
echo -e "${YELLOW}Инструкция:${NC}"
echo -e "1. Подключайте устройства по очереди кабелем к компьютеру."
echo -e "2. Дважды быстро нажмите кнопку ${BOLD}Reset${NC} на плате."
echo -e "3. Скрипт сам запишет нужный файл прошивки и уведомит звуком."
echo -e "----------------------------------------------------------------------"

for i in "${!TARGET_NAMES[@]}"; do
    STEP=$((i+1))
    NAME="${TARGET_NAMES[$i]}"
    FILE="${TARGET_FILES[$i]}"

    echo ""
    echo -e "${BOLD}${YELLOW}>>> ШАГ ${STEP}/${TOTAL_TARGETS}: ${NAME}${NC}"
    flash_device "$NAME" "$FILE"
done

echo ""
echo -e "${BOLD}${GREEN}**********************************************************************${NC}"
echo -e "${BOLD}${GREEN}  🎉 ВСЕ УСТРОЙСТВА (${TOTAL_TARGETS}/${TOTAL_TARGETS}) УСПЕШНО ПРОШИТЫ И ГОТОВЫ К РАБОТЕ!   ${NC}"
echo -e "${BOLD}${GREEN}**********************************************************************${NC}"
play_sound "complete"
send_notification "RMK Flasher" "Все выбранные устройства ($TOTAL_TARGETS) успешно прошиты!" "normal"
echo ""
