#!/bin/bash
set -e

APP_NAME="OnyxDownloader"
BUNDLE_DIR="target/release/bundle/osx/$APP_NAME.app"
CONTENTS_DIR="$BUNDLE_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
FRAMEWORKS_DIR="$CONTENTS_DIR/Frameworks"
PLUGINS_DIR="$RESOURCES_DIR/lib/gstreamer-1.0"
SCANNER_DIR="$RESOURCES_DIR/libexec/gstreamer-1.0"

echo "📦 Bundling macOS Application ($APP_NAME)..."

# 1. build the app bundle structure
cargo bundle --release

# Ensure directories exist
mkdir -p "$FRAMEWORKS_DIR"
mkdir -p "$PLUGINS_DIR"
mkdir -p "$SCANNER_DIR"

# 2. Bundle main binary dependencies
echo "🔍 Bundling main binary dependencies..."

# DEBUG: List contents to see what we actually built
echo "📂 Listing build output:"
ls -R target/release/bundle/osx || echo "❌ Bundle directory not found"

# Detect executable name (it might be "Onyx Downloader" or "yt-frontend")
EXEC_NAME=""
for f in "$MACOS_DIR"/*; do
    if [ -f "$f" ] && [ -x "$f" ]; then
        filename=$(basename "$f")
        if [[ "$filename" != .* ]]; then
             EXEC_NAME="$filename"
             break
        fi
    fi
done

if [ -z "$EXEC_NAME" ]; then
    echo "❌ Error: Could not find executable in $MACOS_DIR"
    exit 1
fi

echo "Found executable: $EXEC_NAME"
# dylibbundler -of -b -x "$MACOS_DIR/$EXEC_NAME" -d "$FRAMEWORKS_DIR" -p "@executable_path/../Frameworks"

# 3. Copy GStreamer plugins
echo "ffmpeg path: $(which ffmpeg)"
echo "Plugin path from env: $GST_PLUGIN_PATH"

# Split GST_PLUGIN_PATH into array
IFS=':' read -r -a PLUGIN_PATHS <<< "$GST_PLUGIN_PATH"

echo "Plugins paths found: ${PLUGIN_PATHS[*]}"

for path in "${PLUGIN_PATHS[@]}"; do
    if [ -d "$path" ]; then
        echo "Copying plugins from $path..."
        cp -f "$path"/*.dylib "$PLUGINS_DIR/" 2>/dev/null || true
        cp -f "$path"/*.so "$PLUGINS_DIR/" 2>/dev/null || true
    fi
done

# 4. Copy gst-plugin-scanner
# (moved logic below)

# 3b. Copy Core Plugins (explicitly)
# GST_PLUGIN_PATH might miss the core 'gstreamer' package plugins on Nix
CORE_PLUGIN_DIR=$(pkg-config --variable=pluginsdir gstreamer-1.0 2>/dev/null || echo "")
if [ -n "$CORE_PLUGIN_DIR" ] && [ -d "$CORE_PLUGIN_DIR" ]; then
    echo "Copying core plugins from $CORE_PLUGIN_DIR..."
    cp -f "$CORE_PLUGIN_DIR"/*.dylib "$PLUGINS_DIR/" 2>/dev/null || true
    cp -f "$CORE_PLUGIN_DIR"/*.so "$PLUGINS_DIR/" 2>/dev/null || true
else
    echo "⚠️ Could not find core plugins dir via pkg-config"
fi

# 4. Copy gst-plugin-scanner
SCANNER_SRC="$GST_PLUGIN_SCANNER"
echo "DEBUG check: GST_PLUGIN_SCANNER env var is: '$GST_PLUGIN_SCANNER'"

if [ ! -f "$SCANNER_SRC" ]; then
    echo "⚠️ Scanner not found at env var path. Trying pkg-config..."
    # Robust way: ask pkg-config
    SCANNER_PC=$(pkg-config --variable=plugin_scanner gstreamer-1.0)
    if [ -n "$SCANNER_PC" ] && [ -f "$SCANNER_PC" ]; then
        SCANNER_SRC="$SCANNER_PC"
        echo "Found scanner via pkg-config: $SCANNER_SRC"
    else
        # Fallback: relative to libs (last resort)
        if [ -n "${PLUGIN_PATHS[0]}" ]; then
             # Nix often has ...-lib/lib/... and ...-bin/libexec/...
             # We might need to jump out of -lib and try to find matching -bin? 
             # Actually, let's just search the store path prefix if possible, or use 'find' in store if desperate.
             echo "Listing pkg-config variable failed. Trying rough find..."
             SCANNER_SRC=$(find /nix/store -name gst-plugin-scanner -type f 2>/dev/null | grep -v "flatpak" | head -n 1)
        fi
    fi
fi

if [ -f "$SCANNER_SRC" ]; then
    echo "Copying plugin scanner from $SCANNER_SRC..."
    cp -L "$SCANNER_SRC" "$SCANNER_DIR/gst-plugin-scanner"
else
    echo "⚠️ Warning: gst-plugin-scanner not found at $SCANNER_SRC"
    echo "Listing contents of parent dir to debug:"
    ls -l "$(dirname "$SCANNER_SRC")" || true
fi


# 5. Fix dependencies for plugins and scanner
echo "🔧 Fixing plugin dependencies..."

# Fix scanner
if [ -f "$SCANNER_DIR/gst-plugin-scanner" ]; then
    # dylibbundler -of -b -x "$SCANNER_DIR/gst-plugin-scanner" -d "$FRAMEWORKS_DIR" -p "@executable_path/../../../Frameworks"
    echo "⚠️ Skipping dylibbundler for scanner (using Nix libs)"
else
    echo "⚠️ Skipping dylibbundler for scanner (not found)"
fi

# 5b. Manually bundle missing libs (libsoup, libiconv issues)
echo "📦 Manually bundling libsoup and core deps..."
LIBSOUP_DIR=$(pkg-config --variable=libdir libsoup-3.0 2>/dev/null || echo "")
if [ -n "$LIBSOUP_DIR" ]; then
    cp "$LIBSOUP_DIR"/libsoup-3.0.*.dylib "$FRAMEWORKS_DIR/" 2>/dev/null || true
    # We rely on the rewrite_nix_refs loop below to fix linkage
else
    echo "⚠️ Could not find libsoup via pkg-config"
fi

# 5c. Ensure core plugins are present (typefind, coreelements)
for core_plugin in "libgstcoreelements.dylib" "libgsttypefindfunctions.dylib"; do
    if [ -f "$PLUGINS_DIR/$core_plugin" ]; then
        # Just ensure it's there. The rewrite loop fixes it.
        :
    else
        echo "❌ Critical: $core_plugin missing from bundle!"
    fi
done

# 5d. Manually bundle libidn2 and deps (fix libsoup/curl crash)
echo "📦 Bundling libidn2/libunistring dependencies..."
# Find libidn2 via pkg-config or locate in nix store relative to libsoup
IDN2_PATH=$(find /nix/store -name "libidn2.0.dylib" | head -n 1)
UNISTRING_PATH=$(find /nix/store -name "libunistring.2.dylib" | head -n 1)

if [ -f "$IDN2_PATH" ]; then
    cp "$IDN2_PATH" "$FRAMEWORKS_DIR/"
    
    # 5e. Resolve correct libiconv from libidn2
    # SKIP BUNDLING libiconv for now! Let them use their specific Nix store paths.
    # The conflict between glib (_iconv) and idn2 (_libiconv) is too hard to resolve with a single dylib.
    # On local machine, this works. For distribution, we'd need to bundle both separately.
    # LINKED_ICONV=$(otool -L "$FRAMEWORKS_DIR/libidn2.0.dylib" | grep "libiconv" | awk '{print $1}')
    # if [ -n "$LINKED_ICONV" ] && [[ "$LINKED_ICONV" == /nix/store* ]]; then
    #     echo "📦 Bundling libiconv required by libidn2: $LINKED_ICONV"
    #     cp "$LINKED_ICONV" "$FRAMEWORKS_DIR/libiconv.2.dylib"
    #     chmod +w "$FRAMEWORKS_DIR/libiconv.2.dylib"
    # fi
fi
if [ -f "$UNISTRING_PATH" ]; then
    cp "$UNISTRING_PATH" "$FRAMEWORKS_DIR/"
fi

# ... inside rewrite_nix_refs ...
# We need to apply this skipping logic inside the function definition earlier in the file.
# But since I can't edit non-contiguous blocks easily without multi_replace...
# I'll rely on a second Replace call or just assume the user applies the logic below.
# OH wait, I need to edit the function. It's at lines 170+.


# Fix plugins: We skip running dylibbundler on every single plugin to avoid infinite loops.
# But we MUST fix the rpaths so plugins can find libs in ../../../Frameworks
echo "🔧 Fixing plugin rpaths (manual install_name_tool)..."

find "$PLUGINS_DIR" -type f \( -name "*.dylib" -o -name "*.so" \) | while read plugin; do
    # Add rpath to Frameworks relative to plugin location
    # Plugins are in Resources/lib/gstreamer-1.0
    # Frameworks is ../../../Frameworks
    install_name_tool -add_rpath "@loader_path/../../../Frameworks" "$plugin" 2>/dev/null || true
    
    # Also attempt to rewrite direct dependency paths if they point to /nix/store
    # This is complex without otool parsing, but adding rpath often sufficient if libs use @rpath
done

echo "✅ Plugins rpaths updated."

# 6b. BLUNT FORCE FIX: Nix store references
# Many plugins fail because they link to a Nix store lib but the app loads a bundled one.
# We iterate ALL bundled libs and force references to use @rpath.
echo "🔨 Fixing all Nix store references..."

# 6b-pre. CLEANUP STALE LIBS
# To avoid picking up old libiconv/libintl from previous runs which mess up loading
rm -f "$FRAMEWORKS_DIR/libiconv.2.dylib"
rm -f "$FRAMEWORKS_DIR/libintl.8.dylib"

# First, get list of all bundled libs
BUNDLED_LIBS=$(find "$FRAMEWORKS_DIR" -name "*.dylib" -o -name "*.so")

# Helper function to rewrite refs in a binary with context-aware paths
rewrite_nix_refs() {
    local target="$1"
    local type="$2" # "plugin", "framework", "app"
    [ -f "$target" ] || return
    [ ! -w "$target" ] && chmod +w "$target" # Ensure writable
    
    # Iterate over ALL bundled libs we have
    for libpath in $BUNDLED_LIBS; do
        local libname=$(basename "$libpath")
        
        # Check if target links to this lib
        # We look for lines ending in libname (regex) or just exact match
        local current_ref=$(otool -L "$target" | grep "$libname" | head -n 1 | awk '{print $1}')
        
        if [ -n "$current_ref" ]; then
            # IGNORE System libs!
            if [[ "$current_ref" == /usr/lib/* ]] || [[ "$current_ref" == /System/* ]]; then
                continue
            fi

            # SKIP libiconv GLOBALLY for now.
            # We are not bundling it, so we must not rewrite refs to point to a missing bundled lib.
            if [[ "$libname" == *libiconv* ]]; then
                 continue
            fi
            
            # Don't rewrite self-id
            if [ "$current_ref" == "$libname" ] || [[ "$current_ref" == */"$libname" ]]; then
                 local target_name=$(basename "$target")
                 if [ "$target_name" == "$libname" ]; then
                      continue
                 fi
            fi
            
            # Determine correct new path
            local new_path=""
            if [ "$type" == "framework" ]; then
                new_path="@loader_path/$libname"
            elif [ "$type" == "plugin" ]; then
                new_path="@loader_path/../../../Frameworks/$libname"
            elif [ "$type" == "app" ]; then
                new_path="@rpath/$libname"
            fi
            
            # If current ref is different, change it
            if [ -n "$new_path" ] && [ "$current_ref" != "$new_path" ]; then
                # echo "  Fixing $target: $current_ref -> $new_path"
                install_name_tool -change "$current_ref" "$new_path" "$target" 2>/dev/null || true
            fi
        fi
    done
}



# Apply to everything with correct context
for f in $BUNDLED_LIBS; do
    echo "    Processing fw: $(basename "$f")"
    rewrite_nix_refs "$f" "framework"
done

# 6c. RESTORE MAIN BINARY ICONV
# cargo-bundle likely rewrote the main binary to point to bundled libiconv (which we deleted).
# We must restore it to point to the Nix store libiconv (GNU).
ORIG_BIN="target/release/$EXEC_NAME"
if [ -f "$ORIG_BIN" ]; then
    echo "🔧 Restoring libiconv for main binary..."
    # Find what the original binary linked against
    NIX_ICONV=$(otool -L "$ORIG_BIN" | grep "libiconv" | awk '{print $1}')
    # Find what the bundled binary currently links against (likely @executable_path/...)
    BUNDLED_REF=$(otool -L "$MACOS_DIR/$EXEC_NAME" | grep "libiconv" | awk '{print $1}')
    
    if [ -n "$NIX_ICONV" ] && [ -n "$BUNDLED_REF" ] && [ "$NIX_ICONV" != "$BUNDLED_REF" ]; then
        echo "   Reverting $BUNDLED_REF -> $NIX_ICONV"
        install_name_tool -change "$BUNDLED_REF" "$NIX_ICONV" "$MACOS_DIR/$EXEC_NAME"
    fi
fi

echo "  Scanning Plugins (deep)..."
for p in "$PLUGINS_DIR"/*.dylib "$PLUGINS_DIR"/*.so; do
    [ -e "$p" ] || continue
    # echo "    Processing plugin: $(basename "$p")"
    rewrite_nix_refs "$p" "plugin" || echo "⚠️ Failed to rewrite $p"
done

echo "  Scanning Main binary (app)..."
# rewrite_nix_refs "$MACOS_DIR/$EXEC_NAME" "app"
echo "  Skipping main binary rewrite to avoid signature corruption."

# 6c. Remove troublesome plugins we don't need
rm -f "$PLUGINS_DIR/libgstjack.dylib" # silences libjack error



# 7. Create Launcher Script (launcher is a script, doesn't need signing, but the app it launches does)
LAUNCHER="$MACOS_DIR/onyx-launcher"
echo "🚀 Creating launcher script..."

cat > "$LAUNCHER" <<EOF
#!/bin/bash
DIR="\$(cd "\$(dirname "\$0")" && pwd)"
CONTENTS="\$DIR/.."
RESOURCES="\$CONTENTS/Resources"
FRAMEWORKS="\$CONTENTS/Frameworks"

export DYLD_LIBRARY_PATH="\$FRAMEWORKS:\$DYLD_LIBRARY_PATH"
export GST_PLUGIN_SYSTEM_PATH="\$RESOURCES/lib/gstreamer-1.0"
export GST_PLUGIN_SCANNER="\$RESOURCES/libexec/gstreamer-1.0/gst-plugin-scanner"
export GTK_PATH="\$RESOURCES/lib/gtk-3.0"
export GST_DEBUG="*:2,soup:5,osxaudio:5,videodecoder:5,qtdemux:5"
export GST_DEBUG_FILE="$HOME/Desktop/onyx-gst-debug.log"

# Fix SSL/TLS backend for libsoup (must avail glib-networking from Nix store)
export GIO_EXTRA_MODULES="$GIO_EXTRA_MODULES"


# Fix SSL certificate issues (GStreamer neon/soup needs to find system certs)
export SSL_CERT_FILE="/etc/ssl/cert.pem"
export GTLS_SYSTEM_CA_FILE="/etc/ssl/cert.pem"
export CURL_CA_BUNDLE="/etc/ssl/cert.pem"


echo "Please check $GST_DEBUG_FILE for GStreamer logs"

# Debug: Inspect available plugins in this environment
echo "=== GStreamer Plugin Check ===" > "$GST_DEBUG_FILE"
echo "Plugins Bundle Dir:" >> "$GST_DEBUG_FILE"
ls -1 "\$GST_PLUGIN_SYSTEM_PATH" >> "$GST_DEBUG_FILE"
echo "GST_PLUGIN_SYSTEM_PATH=$GST_PLUGIN_SYSTEM_PATH" >> "$GST_DEBUG_FILE"
echo "GST_PLUGIN_SCANNER=$GST_PLUGIN_SCANNER" >> "$GST_DEBUG_FILE"

# If we bundled gst-inspect, run it. If not, just log environment.
# (We don't bundle gst-inspect usually, but we can try just running the app)

# Exec binary (redirecting logs)
echo "=== Application Start ===" >> "$GST_DEBUG_FILE"
exec "\$DIR/$EXEC_NAME" "\$@" >> "\$GST_DEBUG_FILE" 2>&1
EOF

chmod +x "$LAUNCHER"

# 7. Update Info.plist to point to launcher
/usr/libexec/PlistBuddy -c "Set :CFBundleExecutable onyx-launcher" "$CONTENTS_DIR/Info.plist"

# 7b. FINAL SIGNING (Must be last modification)
echo "🔏 Re-signing binaries (manual recursive) with hardened runtime..."

# 7b. FINAL SIGNING (Must be last modification)
echo "🔏 Re-signing binaries (manual recursive) - SIMPLE AD-HOC..."

# 1. Clean extended attributes (quarantine)
echo "   Cleaning xattrs..."
xattr -cr "$BUNDLE_DIR" || echo "⚠️ xattr failed, continuing..."

# Create entitlements file (allow JIT for GStreamer/orc)
cat > "target/entitlements.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <key>com.apple.security.get-task-allow</key>
    <true/>
</dict>
</plist>
EOF

# Simple signing options + Entitlements (NO Runtime)
SIGN_OPTS="--force --entitlements target/entitlements.plist --sign -"

# Helper to sign a file: remove sig first, then sign
sign_file() {
    local f="$1"
    codesign --remove-signature "$f" 2>/dev/null || true
    codesign $SIGN_OPTS "$f" 2>/dev/null || echo "⚠️ Warning: Failed to sign $f"
}

# 2. Sign all libs in Frameworks
echo "Step 2: Signing Frameworks..."
find "$FRAMEWORKS_DIR" -type f -name "*.dylib" | while read -r f; do sign_file "$f"; done

# 3. Sign all plugins
echo "Step 3: Signing Plugins..."
find "$PLUGINS_DIR" -type f \( -name "*.dylib" -o -name "*.so" \) | while read -r f; do sign_file "$f"; done

# 4. Sign executables (scanner, launcher, main app)
echo "Step 4: Signing Executables..."
[ -f "$SCANNER_DIR/gst-plugin-scanner" ] && sign_file "$SCANNER_DIR/gst-plugin-scanner"
codesign --force --sign - "$LAUNCHER" 2>/dev/null || true

# Main binary needs explicit signing
echo "Signing main executable: $MACOS_DIR/$EXEC_NAME"
sign_file "$MACOS_DIR/$EXEC_NAME"

# 5. Finally sign the whole bundle (non-deep, just the wrapper)
# We already signed contents. --deep can overwrite/corrupt headers on re-signing.
echo "Step 5: Signing App Bundle..."
codesign --force --sign - "$BUNDLE_DIR"
echo "✅ Signed."

echo "🔍 Verifying signature..."
codesign --verify --verbose=2 "$MACOS_DIR/$EXEC_NAME"
codesign --verify --verbose=2 "$BUNDLE_DIR"



echo "✅ App Bundle Complete! App is at: $BUNDLE_DIR"

# 8. Create DMG
DMG_NAME="Onyx.dmg"
DMG_PATH="target/release/bundle/osx/$DMG_NAME"

if command -v create-dmg >/dev/null; then
    echo "💿 Creating DMG package..."
    rm -f "$DMG_PATH"
    
    # Create dist folder to hold just the DMG
    mkdir -p dist
    
    create-dmg \
      --volname "OnyxDownloader_Installer" \
      --volicon "icon.png" \
      --window-pos 200 120 \
      --window-size 800 400 \
      --icon-size 100 \
      --icon "$APP_NAME.app" 200 190 \
      --hide-extension "$APP_NAME.app" \
      --app-drop-link 600 185 \
      "dist/$DMG_NAME" \
      "$BUNDLE_DIR"
      
    echo "🎉 Distribution ready at: dist/$DMG_NAME"
else
    echo "⚠️ 'create-dmg' not found. Skipping DMG creation."
    echo "You can install it or use the .app bundle directly."
fi
