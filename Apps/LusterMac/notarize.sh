#!/bin/bash
# Builds Luster.app signed with a Developer ID, has Apple notarize it, staples
# the ticket and packs the zip a GitHub release carries. Without a notarized
# app, macOS 15 refuses to open a download until the user overrides it in
# System Settings.
#
#   Apps/LusterMac/notarize.sh
#
# The credentials are a notarytool keychain profile, stored once with
# `xcrun notarytool store-credentials luster-notary …`. LUSTER_NOTARY_PROFILE
# and LUSTER_SIGN_IDENTITY override the profile and the identity. It uploads
# nothing to GitHub; it prints the command that does.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$ROOT/../.." && pwd)"
APP="$ROOT/.build/Luster.app"
PROFILE="${LUSTER_NOTARY_PROFILE:-luster-notary}"
export LUSTER_SIGN_IDENTITY="${LUSTER_SIGN_IDENTITY:-Developer ID Application}"
VERSION="$(grep -m1 '^version = ' "$REPO/rust/Cargo.toml" | cut -d'"' -f2)"
SUBMISSION="$ROOT/.build/Luster-notarize.zip"
ZIP="$ROOT/.build/Luster-$VERSION-macos.zip"

"$ROOT/make-app.sh" release
codesign --verify --strict --verbose=2 "$APP"

# notarytool takes a zip; ditto keeps the bundle's metadata as Finder would.
rm -f "$SUBMISSION"
ditto -c -k --keepParent "$APP" "$SUBMISSION"
RESULT="$(xcrun notarytool submit "$SUBMISSION" --keychain-profile "$PROFILE" \
    --wait --output-format json)"
echo "$RESULT"
STATUS="$(echo "$RESULT" | python3 -c 'import json, sys; print(json.load(sys.stdin)["status"])')"
if [ "$STATUS" != "Accepted" ]; then
    ID="$(echo "$RESULT" | python3 -c 'import json, sys; print(json.load(sys.stdin)["id"])')"
    xcrun notarytool log "$ID" --keychain-profile "$PROFILE" >&2
    echo "notarization: $STATUS" >&2
    exit 1
fi
rm -f "$SUBMISSION"

# The ticket in the bundle lets Gatekeeper pass it without asking Apple.
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute --verbose=2 "$APP"

rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
cat <<EOF

$ZIP is notarized and stapled. To put it on the release:
  gh release upload $VERSION "$ZIP"
EOF
