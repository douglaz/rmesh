{
  description = "rmesh - A comprehensive command-line interface for Meshtastic devices";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
          targets = [ 
            "x86_64-unknown-linux-musl"
            "aarch64-unknown-linux-musl"
          ];
        };
      in
      {
        # Default package: static musl build
        packages.default = let
          rustPlatformMusl = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
        in rustPlatformMusl.buildRustPackage {
          pname = "rmesh";
          version = "0.1.0";
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "meshtastic-0.1.7" = "sha256-OPccMaD8A7IzQjgiOgrTUXKWESHEWkp9Zdo3+xetMM4=";
            };
          };
          
          # Build only the CLI binary
          buildAndTestSubdir = "rmesh";
          
          nativeBuildInputs = with pkgs; [
            pkg-config
            rustToolchain
            pkgsStatic.stdenv.cc
          ];
          
          buildInputs = with pkgs.pkgsStatic; [
            openssl
          ] ++ (with pkgs; lib.optionals stdenv.isLinux [
            udev
          ]);
          
          # Environment variables for static linking
          OPENSSL_STATIC = "1";
          OPENSSL_LIB_DIR = "${pkgs.pkgsStatic.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.pkgsStatic.openssl.dev}/include";
          PKG_CONFIG_PATH = "${pkgs.pkgsStatic.openssl.dev}/lib/pkgconfig";
          
          # Force cargo to use the musl target
          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = 
            "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
          CC_x86_64_unknown_linux_musl = 
            "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
          CARGO_BUILD_RUSTFLAGS = "-C target-feature=+crt-static -C link-arg=-static";
          
          # Override buildPhase to use the correct target
          buildPhase = ''
            runHook preBuild
            
            echo "Building with musl target for static binary..."
            cargo build \
              --release \
              --target x86_64-unknown-linux-musl \
              --offline \
              -j $NIX_BUILD_CORES
            
            runHook postBuild
          '';
          
          installPhase = ''
            runHook preInstall
            
            mkdir -p $out/bin
            cp target/x86_64-unknown-linux-musl/release/rmesh $out/bin/
            
            runHook postInstall
          '';
          
          doCheck = false; # Tests don't work well with static linking
          
          # Verify the binary is statically linked
          postInstall = ''
            echo "Checking if binary is statically linked..."
            file $out/bin/rmesh
            # Strip the binary to reduce size
            ${pkgs.binutils}/bin/strip $out/bin/rmesh
          '';
          
          meta = with pkgs.lib; {
            description = "A comprehensive command-line interface for Meshtastic devices";
            homepage = "https://github.com/douglaz/rmesh";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
          };
        };
        
        # Alternative dynamic build (non-static)
        packages.rmesh-dynamic = pkgs.rustPlatform.buildRustPackage {
          pname = "rmesh-dynamic";
          version = "0.1.0";
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "meshtastic-0.1.7" = "sha256-OPccMaD8A7IzQjgiOgrTUXKWESHEWkp9Zdo3+xetMM4=";
            };
          };
          
          nativeBuildInputs = with pkgs; [
            pkg-config
            rustToolchain
          ];
          
          buildInputs = with pkgs; [
            openssl
          ] ++ lib.optionals stdenv.isLinux [
            udev
          ];
          
          meta = with pkgs.lib; {
            description = "rmesh (dynamic build)";
            homepage = "https://github.com/douglaz/rmesh";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
          };
        };
        
        # Development shell
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            bashInteractive
            rustToolchain
            pkg-config
            pkgsStatic.stdenv.cc
            openssl
            pkgsStatic.openssl
            cargo-edit
            cargo-outdated
            cargo-watch
            rust-analyzer
            gh
          ] ++ lib.optionals stdenv.isLinux [
            udev
          ];

          CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = 
            "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
          CC_x86_64_unknown_linux_musl = 
            "${pkgs.pkgsStatic.stdenv.cc}/bin/${pkgs.pkgsStatic.stdenv.cc.targetPrefix}cc";
          
          # For static linking with musl
          OPENSSL_STATIC = "1";
          OPENSSL_LIB_DIR = "${pkgs.pkgsStatic.openssl.out}/lib";
          OPENSSL_INCLUDE_DIR = "${pkgs.pkgsStatic.openssl.dev}/include";
          PKG_CONFIG_PATH = "${pkgs.pkgsStatic.openssl.dev}/lib/pkgconfig";
          
          shellHook = ''
            echo "🔧 Meshtastic CLI Development Environment"
            echo ""
            echo "Available commands:"
            echo "  cargo build                - Build debug version"
            echo "  cargo build --release      - Build optimized version"
            echo "  cargo run -- --help        - Run CLI with help"
            echo "  cargo test                 - Run tests"
            echo "  cargo clippy               - Run linter"
            echo ""
            echo "Build static binary:"
            echo "  cargo build --release --target x86_64-unknown-linux-musl"
            echo ""
          '';
        };
      }
    );
}