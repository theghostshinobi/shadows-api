#!/bin/sh
# Costruisce Shadow.app — l'applicazione della barra dei menu di macOS.
#
# Niente Xcode e niente progetto: swiftc e un bundle costruito a mano. Su uno
# strumento che deve girare anche su un server, meno macchinario c'e' meglio e'.
#
#   sh macos/build.sh              -> macos/build/Shadow.app
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/macos/build"
app="$out/Shadow.app"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Shadow</string>
  <key>CFBundleDisplayName</key><string>Shadow</string>
  <key>CFBundleIdentifier</key><string>dev.shadow.menubar</string>
  <key>CFBundleVersion</key><string>0.0.0</string>
  <key>CFBundleShortVersionString</key><string>0.0.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>ShadowMenuBar</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <!-- Vive nella barra dei menu: nessuna icona nel Dock, nessuna finestra
       all'avvio. -->
  <key>LSUIElement</key><true/>
  <!-- Nessuna chiave di rete: questa applicazione non parla con nessuno.
       Legge chiamando il binario `shadow`, che e' sulla stessa macchina. -->
</dict>
</plist>
PLIST

swiftc -O \
  -o "$app/Contents/MacOS/ShadowMenuBar" \
  -framework AppKit -framework SwiftUI -framework WebKit \
  "$root/macos/ShadowMenuBar/main.swift"

echo "costruita: $app"
echo
echo "Prima di aprirla, dille dove guardare:"
echo "  ~/Library/Application Support/Shadow/menubar.json"
echo "  (se non c'e', l'applicazione ne scrive uno di esempio al primo avvio)"
