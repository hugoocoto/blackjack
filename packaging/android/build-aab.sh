#!/bin/sh
# Turns the APK that cargo quad-apk builds into an Android App Bundle (.aab),
# the only format Google Play takes for new apps; cargo quad-apk can't make
# one. aapt2 converts the APK's manifest and resources to the bundle's
# (protobuf) format, then bundletool packs them with the code and libraries.
#
# Usage: build-aab.sh APK AAB
#
# Needs ANDROID_HOME (for aapt2, from the newest build-tools there), java,
# zip, unzip and BUNDLETOOL (the path to bundletool-all.jar). Signs the bundle
# with the upload key when ANDROID_KEYSTORE (the keystore file),
# ANDROID_KEYSTORE_PASSWORD and ANDROID_KEY_ALIAS are set;
# ANDROID_KEY_PASSWORD defaults to the keystore's password.
set -e

if [ $# -ne 2 ]; then
    echo "Usage: $0 APK AAB" >&2
    exit 1
fi
apk=$(realpath "$1")
aab=$(realpath "$2")

build_tools=$(ls "$ANDROID_HOME/build-tools" | sort -V | tail -1)
aapt2="$ANDROID_HOME/build-tools/$build_tools/aapt2"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

"$aapt2" convert --output-format proto -o "$work/proto.apk" "$apk"
mkdir "$work/apk" "$work/module"
unzip -q "$work/proto.apk" -d "$work/apk"

# The module layout bundletool wants: the manifest and the code in their own
# folders, files it doesn't know under root/, and no signature (META-INF).
cd "$work/apk"
for entry in *; do
    case $entry in
        AndroidManifest.xml) mkdir -p ../module/manifest && mv "$entry" ../module/manifest/ ;;
        classes*.dex) mkdir -p ../module/dex && mv "$entry" ../module/dex/ ;;
        assets | lib | res | resources.pb) mv "$entry" ../module/ ;;
        META-INF) ;;
        *) mkdir -p ../module/root && mv "$entry" ../module/root/ ;;
    esac
done
cd ../module
zip -qr ../base.zip .

rm -f "$aab"
java -jar "$BUNDLETOOL" build-bundle --modules="$work/base.zip" --output="$aab"

if [ -n "$ANDROID_KEYSTORE" ]; then
    export ANDROID_KEY_PASSWORD="${ANDROID_KEY_PASSWORD:-$ANDROID_KEYSTORE_PASSWORD}"
    # :env reads the passwords from the environment, off the command line.
    jarsigner -keystore "$ANDROID_KEYSTORE" \
        -storepass:env ANDROID_KEYSTORE_PASSWORD -keypass:env ANDROID_KEY_PASSWORD \
        "$aab" "$ANDROID_KEY_ALIAS"
    jarsigner -verify "$aab" | grep -q "jar verified"
    echo "Signed $aab with the key $ANDROID_KEY_ALIAS."
else
    echo "ANDROID_KEYSTORE isn't set, so $aab is unsigned; Google Play won't take it." >&2
fi
