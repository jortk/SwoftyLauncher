{
  description = "Swofty Launcher for F1 25 development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAll (
        pkgs:
        let
          runtimeLibs = with pkgs; [
            vulkan-loader
            wayland
            libxkbcommon
            libGL
            libx11
            libxcb
            libxcursor
            libxi
            libxrandr
            libxext
            fontconfig
            freetype
            alsa-lib
            gtk3
            webkitgtk_4_1
            libsoup_3
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustup
              pkg-config
              clang
              cmake
              mold
              zig
              nodejs_24
              pnpm
              python3
              zstd
              openssl
              xvfb-run
              wineWow64Packages.stable
            ];
            buildInputs = runtimeLibs ++ [ pkgs.zstd pkgs.openssl ];
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            # HTTPS in the swofty-ads helper's WebKitGTK.
            GIO_EXTRA_MODULES = "${pkgs.glib-networking}/lib/gio/modules";
          };
        }
      );
    };
}
