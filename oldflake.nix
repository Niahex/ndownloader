{
  description = "nwidgets - A GPUI-based application launcher";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    crane,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        # Overlays and package set
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {inherit system overlays;};

        # Rust toolchain configuration
        rustToolchain = pkgs.rust-bin.stable."1.88.0".default.override {
          extensions = ["rust-src"];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        unfilteredRoot = ./.;
        src = pkgs.lib.fileset.toSource {
          root = unfilteredRoot;
          fileset = pkgs.lib.fileset.unions [
            (craneLib.fileset.commonCargoSources unfilteredRoot)
            (pkgs.lib.fileset.fileFilter (
                file:
                  pkgs.lib.any file.hasExt [
                    "svg"
                  ]
              )
              unfilteredRoot)
            (pkgs.lib.fileset.maybeMissing ./assets)
          ];
        };

        # Dependencies for building the application
        buildInputs = with pkgs; [
          wayland
          vulkan-loader
          vulkan-validation-layers
          vulkan-tools
          mesa
          mesa.drivers
          xorg.libxcb
          xorg.libX11
          libxkbcommon
          fontconfig
          dbus
          openssl
          freetype
          expat
          libnotify
          alsa-lib
          udev
        ];

        # Dependencies needed only at runtime
        runtimeDependencies = with pkgs; [
          vulkan-loader
          mesa
          mesa.drivers
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
          makeWrapper
          autoPatchelfHook
        ];

        envVars = {
          RUST_BACKTRACE = "full";
        };

        # Build artifacts
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src buildInputs nativeBuildInputs;
          env = envVars;
        };

        # Application package definition
        nwidgets = craneLib.buildPackage {
          inherit src cargoArtifacts buildInputs nativeBuildInputs runtimeDependencies;
          env = envVars;
          pname = "nwidgets";
          version = "0.1.0";
        };

        # Development shell tools
        devTools = with pkgs; [
          rust-analyzer
          rustToolchain
          cargo-watch
          cargo-edit
          bacon
        ];
      in {
        packages = {
          default = nwidgets;
          inherit nwidgets;
        };

        checks = {
          inherit nwidgets;

          nwidgets-clippy = craneLib.cargoClippy {
            inherit src cargoArtifacts buildInputs nativeBuildInputs;
            env = envVars;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          };

          nwidgets-fmt = craneLib.cargoFmt {inherit src;};
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = [nwidgets];
          nativeBuildInputs = devTools;
          env = envVars;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (buildInputs ++ runtimeDependencies);
          FONTCONFIG_FILE = pkgs.makeFontsConf {fontDirectories = buildInputs;};
          VK_DRIVER_FILES = "${pkgs.mesa.drivers}/share/vulkan/icd.d/radeon_icd.x86_64.json";
          VK_ICD_FILENAMES = "${pkgs.mesa.drivers}/share/vulkan/icd.d/radeon_icd.x86_64.json";

          shellHook = ''
            echo "[🦀 Rust $(rustc --version)] - Ready to develop nwidgets!"
            echo "Vulkan ICD: $VK_ICD_FILENAMES"
            echo "Available Vulkan devices:"
            vulkaninfo --summary 2>/dev/null | grep -A 2 "GPU" || echo "  Run 'vulkaninfo' for details"
          '';
        };

        formatter = pkgs.alejandra;
      }
    );
}
