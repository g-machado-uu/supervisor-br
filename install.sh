#!/usr/bin/env bash
# Instala o Supervisor BR para o usuário atual no Linux (sem root).
#
#   ./install.sh                       compila, instala e mantém o arquivo de dados atual
#   ./install.sh --path=~/Documentos/  usa também ~/Documentos/data.svt como arquivo de dados
#   ./install.sh --path=~/sync/x.svt   ...ou um nome de arquivo específico
#   ./install.sh --uninstall           remove o binário, o atalho e o ícone (mantém os dados)
#
# Instala:
#   ~/.local/bin/supervisor-br
#   ~/.local/share/applications/supervisor-br.desktop   (Rofi drun, menus de apps)
#   ~/.local/share/icons/hicolor/*/apps/supervisor-br.{svg,png}

set -euo pipefail

APP=supervisor-br
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
APPS_DIR="$DATA_HOME/applications"
ICON_DIR="$DATA_HOME/icons/hicolor"
# Must match storage.rs (directories::ProjectDirs "Supervisor-BR" on Linux: "supervisor-br").
DEFAULT_DATA="$DATA_HOME/$APP/data.svt"
LAST_FILE="$CONFIG_HOME/$APP/last_file.txt"
KEY_FILE="$CONFIG_HOME/$APP/key.bin"

data_path=""
uninstall=0

usage() {
    sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --path=*) data_path="${1#--path=}" ;;
        --path)
            [[ $# -ge 2 ]] || { echo "erro: --path precisa de um valor" >&2; exit 1; }
            data_path="$2"; shift ;;
        --uninstall) uninstall=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "erro: opção desconhecida '$1'" >&2; usage >&2; exit 1 ;;
    esac
    shift
done

refresh_caches() {
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$ICON_DIR" 2>/dev/null || true
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$APPS_DIR" 2>/dev/null || true
}

if [[ $uninstall -eq 1 ]]; then
    rm -f "$BIN_DIR/$APP" "$APPS_DIR/$APP.desktop"
    rm -f "$ICON_DIR"/scalable/apps/$APP.svg "$ICON_DIR"/*/apps/$APP.png
    refresh_caches
    echo "Supervisor BR removido. O arquivo de dados, o arquivo-chave (se houver) e $LAST_FILE foram mantidos."
    exit 0
fi

# --- Resolve the data file ------------------------------------------------
if [[ -n "$data_path" ]]; then
    # The shell doesn't expand ~ after "--path=", so do it here.
    case "$data_path" in
        "~") data_path="$HOME" ;;
        "~/"*) data_path="$HOME/${data_path#\~/}" ;;
    esac
    # A directory (existing, or written with a trailing slash) gets data.svt inside it.
    if [[ "$data_path" == */ || -d "$data_path" ]]; then
        data_path="${data_path%/}/data.svt"
    fi
    data_path="$(realpath -m -- "$data_path")"
    mkdir -p -- "$(dirname -- "$data_path")"
elif [[ -s "$LAST_FILE" ]]; then
    data_path="$(head -n1 "$LAST_FILE")"
else
    data_path="$DEFAULT_DATA"
fi

# --- Build -----------------------------------------------------------------
binary="$SRC_DIR/target/release/$APP"
if command -v cargo >/dev/null; then
    echo "==> Compilando o binário de release"
    cargo build --release --manifest-path "$SRC_DIR/Cargo.toml"
elif [[ ! -x "$binary" ]]; then
    echo "erro: cargo não encontrado e não há $binary pré-compilado" >&2
    exit 1
fi

# --- Install ---------------------------------------------------------------
echo "==> Instalando"
install -Dm755 "$binary" "$BIN_DIR/$APP"

install -Dm644 "$SRC_DIR/packaging/$APP.svg" "$ICON_DIR/scalable/apps/$APP.svg"
# PNGs too, for launchers/themes that don't render SVG.
renderer=""
if command -v magick >/dev/null; then renderer=magick
elif command -v convert >/dev/null; then renderer=convert
fi
if [[ -n "$renderer" ]]; then
    for size in 48 64 128 256; do
        mkdir -p "$ICON_DIR/${size}x${size}/apps"
        "$renderer" -background none "$SRC_DIR/packaging/$APP.svg" -resize "${size}x${size}" \
            "$ICON_DIR/${size}x${size}/apps/$APP.png" 2>/dev/null || true
    done
elif command -v rsvg-convert >/dev/null; then
    for size in 48 64 128 256; do
        mkdir -p "$ICON_DIR/${size}x${size}/apps"
        rsvg-convert -w "$size" -h "$size" "$SRC_DIR/packaging/$APP.svg" \
            -o "$ICON_DIR/${size}x${size}/apps/$APP.png" || true
    done
fi

# Absolute Exec path: Hyprland's exec environment may not include ~/.local/bin.
mkdir -p "$APPS_DIR"
sed -e "s|^Exec=.*|Exec=$BIN_DIR/$APP|" -e "s|^Icon=.*|Icon=$APP|" \
    "$SRC_DIR/packaging/$APP.desktop" > "$APPS_DIR/$APP.desktop"
chmod 644 "$APPS_DIR/$APP.desktop"

# The app opens the last-used file, so pointing that at the chosen path makes
# both the launcher and a bare `supervisor-br` in a terminal use it.
# Owner-only: this folder also holds the key file, if you add one.
mkdir -p -m 700 "$(dirname -- "$LAST_FILE")"
printf '%s' "$data_path" > "$LAST_FILE"

refresh_caches

# --- Report ----------------------------------------------------------------
cat <<EOF

Supervisor BR instalado.

  Binário:   $BIN_DIR/$APP
  Atalho:    $APPS_DIR/$APP.desktop
  Ícone:     $ICON_DIR/scalable/apps/$APP.svg
  Dados:     $data_path
EOF
if [[ -e "$data_path" ]]; then
    echo "             (arquivo existente – desbloqueie com a sua senha)"
else
    echo "             (ainda não existe – crie uma senha na primeira execução)"
fi
echo "             Backup: $data_path.bak (mantido a cada salvamento)"
if [[ -e "$KEY_FILE" ]]; then
    echo "  Arquivo-chave: $KEY_FILE"
fi

case ":$PATH:" in
    *":$BIN_DIR:"*)
        echo
        echo "$BIN_DIR já está no seu PATH – rode 'supervisor-br' num terminal." ;;
    *)
        cat <<EOF

$BIN_DIR não está no seu PATH. Adicione-o no seu shell e abra um novo terminal:

  bash:  echo 'export PATH="\$HOME/.local/bin:\$PATH"' >> ~/.bashrc
  zsh:   echo 'export PATH="\$HOME/.local/bin:\$PATH"' >> ~/.zshrc
  fish:  fish_add_path ~/.local/bin
EOF
        ;;
esac

cat <<EOF

Para abrir:
  Rofi:      rofi -show drun -show-icons   (busque "Supervisor BR")
  Hyprland:  bind = \$mainMod, S, exec, $BIN_DIR/$APP
  Classe da janela (windowrules do Hyprland): supervisor-br
EOF
