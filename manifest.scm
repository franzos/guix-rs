(use-modules (guix packages)
             (guix search-paths)
             (gnu packages rust)
             (gnu packages commencement)
             (gnu packages pkg-config)
             (gnu packages tls)
             (gnu packages base)
             ;; M5 — guix-gui (Iced 0.13 on wgpu): graphics + windowing
             ;; stack. Iced's default wgpu backend needs Vulkan at run
             ;; time and links libxkbcommon / wayland at build time.
             ;; libX11 / libXcursor / libXi / libXrandr cover the X11
             ;; fallback path. Fontconfig + freetype are needed by
             ;; iced_graphics' text shaper.
             (gnu packages xorg)
             (gnu packages xdisorg)
             (gnu packages freedesktop)
             (gnu packages gl)
             (gnu packages vulkan)
             (gnu packages fontutils))

(define openssl-with-dir
  (package
    (inherit openssl)
    (native-search-paths
     (cons (search-path-specification
            (variable "OPENSSL_DIR")
            (files '("."))
            (file-type 'directory)
            (separator #f))
           (package-native-search-paths openssl)))))

(define gcc-toolchain-with-cc
  (package
    (inherit gcc-toolchain)
    (native-search-paths
     (cons (search-path-specification
            (variable "CC")
            (files '("bin/gcc"))
            (file-type 'regular)
            (separator #f))
           (package-native-search-paths gcc-toolchain)))))

(packages->manifest
 (list rust
       (list rust "cargo")
       (list rust "tools")              ; clippy, rustfmt
       rust-analyzer
       gcc-toolchain-with-cc
       pkg-config
       openssl-with-dir
       ;; guix-gui runtime + build deps (Iced 0.13 / wgpu).
       libxkbcommon
       wayland
       wayland-protocols
       vulkan-loader
       mesa
       fontconfig
       freetype
       libx11
       libxcb
       libxcursor
       libxi
       libxrandr))
