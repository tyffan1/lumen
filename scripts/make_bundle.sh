#!/usr/bin/env bash
set -euo pipefail

# === Сборка Lumen.app ===
#
# Использование:
#   ./scripts/make_bundle.sh             # debug-сборка
#   ./scripts/make_bundle.sh --release   # release-сборка
#
# Альтернатива (рекомендуется):
#   cargo install cargo-bundle2
#   cargo bundle --release --bin lumen   # использует [package.metadata.bundle] из Cargo.toml

DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$DIR"

PROFILE="${1:-debug}"
if [[ "$PROFILE" == "--release" ]]; then
    PROFILE_DIR="release"
    CARGO_FLAGS="--release"
else
    PROFILE_DIR="debug"
    CARGO_FLAGS=""
fi

echo "→ Сборка бинарника (cargo build $CARGO_FLAGS --bin lumen)..."
cargo build $CARGO_FLAGS --bin lumen

BUNDLE="$DIR/target/$PROFILE_DIR/Lumen.app"
echo "→ Создание $BUNDLE ..."
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS"
mkdir -p "$BUNDLE/Contents/Resources"

cp "target/$PROFILE_DIR/lumen" "$BUNDLE/Contents/MacOS/lumen"
cp "app/icons/lumen.icns" "$BUNDLE/Contents/Resources/icon.icns"

cat > "$BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>lumen</string>
    <key>CFBundleIdentifier</key>
    <string>com.lumenapp.Lumen</string>
    <key>CFBundleName</key>
    <string>Lumen</string>
    <key>CFBundleDisplayName</key>
    <string>Lumen</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo "✓ Готово: $BUNDLE"
echo "  Запуск: open $BUNDLE"
echo ""
echo "⚠ Если Accessibility Permission не работает:"
echo "   System Settings → Privacy & Security → Accessibility →"
echo "   добавьте Lumen.app (не голый lumen, а .app бандл!)"
