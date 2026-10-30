#!/usr/bin/env bash
# Lanzador de Montworld.
# Compila en modo release si hay cambios y arranca el juego.
# Toda la salida se guarda en /tmp/montworld.log para diagnosticar.
#
# Nota: este script es solo para escritorio. El APK de Android se
# compila aparte y unicamente cuando el usuario lo pide.

set -uo pipefail

PROJECT_DIR="/media/nahuel/Datos/Juegos/Montworld"
LOG="/tmp/montworld.log"
export PATH="$HOME/.cargo/bin:$PATH"

cd "$PROJECT_DIR" || exit 1

{
    echo "=== $(date '+%Y-%m-%d %H:%M:%S') lanzando Montworld ==="
    cargo build --release --quiet || {
        echo "ERROR: fallo la compilacion"
        command -v notify-send >/dev/null &&
            notify-send -i dialog-error "Montworld" "Fallo al compilar. Revisa $LOG"
        exit 1
    }
    exec "$PROJECT_DIR/target/release/montworld" "$@"
} >>"$LOG" 2>&1
