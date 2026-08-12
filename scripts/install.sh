#!/usr/bin/env bash
set -e

# Hitta projektets rotmapp oavsett varifrån skriptet körs
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/src-tauri/target/release/videoclipper"
ICON="$PROJECT_DIR/src-tauri/icons/128x128.png"

echo "=== VideoClipper Installer (Linux / Arch) ==="

# Om release-binären saknas, fråga eller bygg den
if [ ! -f "$BINARY" ]; then
    echo "Release-binären finns inte ännu. Bygger applikationen med 'pnpm tauri build'..."
    cd "$PROJECT_DIR"
    pnpm tauri build
fi

echo "Skapar installationsmappar..."
mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.local/share/icons/hicolor/128x128/apps"
mkdir -p "$HOME/.local/share/applications"

echo "Kopierar programfil och ikon..."
cp "$BINARY" "$HOME/.local/bin/videoclipper"
chmod +x "$HOME/.local/bin/videoclipper"
cp "$ICON" "$HOME/.local/share/icons/hicolor/128x128/apps/videoclipper.png"

echo "Skapar genväg (.desktop-fil) i startmenyn..."
cat << EOF > "$HOME/.local/share/applications/videoclipper.desktop"
[Desktop Entry]
Name=VideoClipper
Comment=Video Clipper Application
Exec=$HOME/.local/bin/videoclipper
Icon=videoclipper
Type=Application
Terminal=false
Categories=AudioVideo;Utility;
EOF

if command -v update-desktop-database &> /dev/null; then
    echo "Uppdaterar startmenyns databas..."
    update-desktop-database "$HOME/.local/share/applications" || true
fi

echo ""
echo "✅ Installationen är klar!"
echo "Appen finns nu installerad i: $HOME/.local/bin/videoclipper"
echo "Du kan nu hitta VideoClipper i din startmeny."
