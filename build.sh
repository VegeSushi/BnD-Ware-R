#!/usr/bin/env bash
# Linux equivalent of build.bat:
#   1. fetch/refresh core\ (assets + minigames), unless --skipfetch
#   2. cargo build --release (bnd_game + bnd_mod_manager)
#   3. package everything into an AppImage (Output/BnDWare-x86_64.AppImage)
set -euo pipefail
cd "$(dirname "$0")"

echo "==============================================="
echo " BnD-Ware Full Build (Linux)"
echo "==============================================="

# ---------------------------------------------------------
# 0. Sanity check tools
# ---------------------------------------------------------
if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo not found on PATH. Install Rust from https://rustup.rs"
    exit 1
fi

# ---------------------------------------------------------
# 1. Fetch/refresh game assets and minigames into ./core
#    (skipped if core/ already exists and --skipfetch was passed)
# ---------------------------------------------------------
if [ "${1:-}" = "--skipfetch" ]; then
    echo "Skipping fetch step, using existing ./core"
else
    if [ ! -d core ]; then
        echo "Running fetch.sh to download assets/minigames..."
        ./fetch.sh
    else
        echo "core/ already exists, skipping fetch. Delete it or pass no args to force re-fetch."
    fi
fi

# ---------------------------------------------------------
# 2. Build game + mod manager (release)
# ---------------------------------------------------------
echo
echo "Building bnd_game and bnd_mod_manager (release)..."
cargo build --release --bin bnd_game --bin bnd_mod_manager

if [ ! -f target/release/bnd_game ]; then
    echo "ERROR: target/release/bnd_game was not produced."
    exit 1
fi
if [ ! -f target/release/bnd_mod_manager ]; then
    echo "ERROR: target/release/bnd_mod_manager was not produced."
    exit 1
fi

# ---------------------------------------------------------
# 3. Package into an AppImage
# ---------------------------------------------------------
echo
echo "Assembling AppDir..."

APPDIR="Output/BnDWare.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"

cp target/release/bnd_game "$APPDIR/usr/bin/"
cp target/release/bnd_mod_manager "$APPDIR/usr/bin/"

# Engine expects a "core" folder sitting next to the executable at runtime
# (see resolve_path()/core_root in src/main.rs), so it travels inside the AppDir.
if [ -d core ]; then
    cp -a core "$APPDIR/usr/bin/core"
else
    echo "WARNING: ./core not found; AppImage will be missing game assets."
fi

# Icon: appimagetool hard-requires bndware.png (or .svg/.xpm) to exist,
# so we must always end up with a real image file here, never a stub.
if [ -f assets/game_icon.ico ] && command -v convert >/dev/null 2>&1; then
    convert assets/game_icon.ico -resize 256x256 "$APPDIR/bndware.png" || true
fi
if [ ! -f "$APPDIR/bndware.png" ]; then
    echo "No usable game_icon.ico/ImageMagick found; generating a placeholder PNG icon..."
    python3 - "$APPDIR/bndware.png" <<'PYEOF'
import struct, zlib, sys

out_path = sys.argv[1]
size = 256
# Solid dark-purple square as a stand-in icon; replace assets/game_icon.ico
# with a proper icon later for a nicer AppImage.
r, g, b = 60, 40, 90
raw = bytearray()
for _ in range(size):
    raw.append(0)  # filter type: None
    for _ in range(size):
        raw += bytes((r, g, b, 255))

def chunk(tag, data):
    return (struct.pack(">I", len(data)) + tag + data +
            struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff))

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
png += chunk(b"IEND", b"")

with open(out_path, "wb") as f:
    f.write(png)
PYEOF
fi

cat > "$APPDIR/bndware.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=BnD-Ware
Comment=BnD-Ware (Rust Engine)
Exec=bnd_game
Icon=bndware
Categories=Game;
Terminal=false
EOF

cat > "$APPDIR/AppRun" <<'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
cd "${HERE}/usr/bin"
exec ./bnd_game "$@"
EOF
chmod +x "$APPDIR/AppRun"

echo "Fetching appimagetool..."
APPIMAGETOOL="Output/appimagetool.AppImage"
if [ ! -f "$APPIMAGETOOL" ]; then
    curl -L -o "$APPIMAGETOOL" \
        https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x "$APPIMAGETOOL"
fi

echo "Building AppImage..."
ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" \
    "Output/BnDWare-x86_64.AppImage"

echo
echo "==============================================="
echo " Build complete!"
echo " Game:        target/release/bnd_game"
echo " Mod Manager: target/release/bnd_mod_manager"
echo " AppImage:    Output/BnDWare-x86_64.AppImage"
echo "==============================================="
