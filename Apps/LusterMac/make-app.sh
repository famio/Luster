#!/bin/bash
# Assembles Luster.app around the SwiftPM binary. The bundle provides the
# Dock tile, the menu bar name, and the Finder association for SVG files.
# Usage: Apps/LusterMac/make-app.sh [debug|release]   (default: release)
set -euo pipefail

CONFIGURATION="${1:-release}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$ROOT/../.." && pwd)"
APP="$ROOT/.build/Luster.app"

swift build -c "$CONFIGURATION" --package-path "$ROOT"
BINARY="$(swift build -c "$CONFIGURATION" --package-path "$ROOT" --show-bin-path)/Luster"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/Luster"
# The licenses travel with the app: MIT requires the notices in every copy.
cp "$REPO/LICENSE" "$REPO/THIRD_PARTY_NOTICES.md" "$APP/Contents/Resources/"
# The strings are English in the code; the catalog holds the other languages.
xcrun xcstringstool compile "$ROOT/Resources/Localizable.xcstrings" \
    --output-directory "$APP/Contents/Resources"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key>                  <string>Luster</string>
	<key>CFBundleDisplayName</key>           <string>Luster</string>
	<key>CFBundleExecutable</key>            <string>Luster</string>
	<key>CFBundleIdentifier</key>            <string>dev.famio.luster</string>
	<key>CFBundleDevelopmentRegion</key>     <string>en</string>
	<key>CFBundleLocalizations</key>         <array><string>en</string><string>ja</string></array>
	<key>CFBundleInfoDictionaryVersion</key> <string>6.0</string>
	<key>CFBundlePackageType</key>           <string>APPL</string>
	<key>CFBundleShortVersionString</key>    <string>1.0</string>
	<key>CFBundleVersion</key>               <string>1</string>
	<key>LSMinimumSystemVersion</key>        <string>15.0</string>
	<key>NSHighResolutionCapable</key>       <true/>
	<key>NSPrincipalClass</key>              <string>NSApplication</string>
	<key>CFBundleDocumentTypes</key>
	<array>
		<dict>
			<key>CFBundleTypeName</key>     <string>Scalable Vector Graphics</string>
			<key>CFBundleTypeRole</key>     <string>Viewer</string>
			<key>LSHandlerRank</key>        <string>Alternate</string>
			<key>LSItemContentTypes</key>
			<array><string>public.svg-image</string></array>
		</dict>
	</array>
</dict>
</plist>
PLIST

# Ad-hoc signature, enough for a locally built app to run. A signing
# failure aborts the script: an unsigned app may be refused at launch.
codesign --force --sign - "$APP"

# A running copy keeps its old code; warn, and print the build timestamp.
if pgrep -x Luster >/dev/null; then
    echo "note: Luster is running; quit it before launching the new build."
fi
echo "built $APP  ($(date -r "$APP/Contents/MacOS/Luster" '+%Y-%m-%d %H:%M:%S'))"
