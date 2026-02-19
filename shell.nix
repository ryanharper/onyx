{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    cargo
    rustc
    rustfmt
    pkg-config
    fontconfig
    freetype
    expat
    openssl
    glib
    cairo
    pango
    atk
    gdk-pixbuf
    glib-networking
    cacert # CA Certificates for SSL/TLS
    # Emoji font support
    noto-fonts-color-emoji
    # Tools needed for downloading dependencies
    curl
    gnutar
    xz
    cargo-bundle
    # flatpak # Use host flatpak for runtime management to avoid D-Bus/permission issues
    flatpak-builder
    # JavaScript runtime for yt-dlp YouTube extraction
    deno
    # ffmpeg for video processing (needed for time range downloads)
    ffmpeg
    # mpv for video player
    mpv
    # GStreamer for iced_video_player
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
    gst_all_1.gst-plugins-bad
    gst_all_1.gst-plugins-ugly
    gst_all_1.gst-libav
    libsoup_3
  ] ++ (with pkgs; lib.optionals stdenv.isLinux [
    libxkbcommon
    vulkan-loader
    wayland
    xorg.libX11
    xorg.libXcursor
    xorg.libXi
    xorg.libXrandr
    gtk3
    flatpak
    flatpak-builder
  ]) ++ (with pkgs; lib.optionals stdenv.isDarwin [
    libiconv
    apple-sdk_26
    macdylibbundler
    create-dmg
  ]);

  LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath ([
    pkgs.glib
    pkgs.cairo
    pkgs.pango
    pkgs.atk
    pkgs.gdk-pixbuf
    pkgs.gst_all_1.gstreamer
    pkgs.gst_all_1.gst-plugins-base
  ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
    pkgs.vulkan-loader
    pkgs.libxkbcommon
    pkgs.wayland
    pkgs.xorg.libX11
    pkgs.xorg.libXcursor
    pkgs.xorg.libXi
    pkgs.xorg.libXrandr
    pkgs.gtk3
  ])}";

  shellHook = ''
    # Force clean GStreamer registry scan
    rm -f $HOME/.cache/gstreamer-1.0/registry.x86_64.bin

    # Use GST_PLUGIN_PATH to augment/override search paths
    export GST_PLUGIN_PATH="${pkgs.lib.makeSearchPath "lib/gstreamer-1.0" (with pkgs.gst_all_1; [ gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav ])}"
    
    export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules"
    export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
    export GST_PLUGIN_SCANNER="${pkgs.gst_all_1.gstreamer}/libexec/gstreamer-1.0/gst-plugin-scanner"
    export GST_DEBUG=3 # Enable debug logs for GStreamer
  '';
}
