{
  description = "重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";

    templates-src = {
      url = "github:tuack-ng/templates";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
      templates-src,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.workspace.package.version;

        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        lib = pkgs.lib;

        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.gitTracked ./.;
        };

        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          pname = "tuack-ng";
          inherit version;
          cargoExtraArgs = "-p tuack-ng --locked --no-default-features --features=nix";
        };

        cargoArtifactsRpc = craneLib.buildDepsOnly {
          inherit src;
          pname = "tuack-ng-rpc";
          inherit version;
          cargoExtraArgs = "-p tuack-ng-rpc --locked --no-default-features --features=nix";
        };

        tuack-ng = craneLib.buildPackage {
          inherit src cargoArtifacts;
          pname = "tuack-ng";
          inherit version;

          cargoExtraArgs = "-p tuack-ng --locked --no-default-features --features=nix";

          nativeBuildInputs = with pkgs; [
            gcc
            installShellFiles
            testlib
          ];

          buildInputs = with pkgs; [
            testlib
          ];

          NIX_TESTLIB_PATH = "${pkgs.testlib}/include/testlib/testlib.h";
          VERGEN_IDEMPOTENT = 1;

          postInstall = ''
            # 安装静态资源
            mkdir -p $out/share/tuack-ng
            cp -r assets/* $out/share/tuack-ng/

            # 使用系统的 testlib
            ln -sf ${pkgs.testlib}/include/testlib/testlib.h \
              $out/share/tuack-ng/checkers/testlib.h

            # 安装 templates
            mkdir -p $out/share/tuack-ng/templates
            cp -r ${templates-src}/* \
              $out/share/tuack-ng/templates/
            chmod -R u+w $out/share/tuack-ng/templates/

            # 生成 shell 补全
            $out/bin/tuack-ng gen complete bash > tuack-ng.bash
            $out/bin/tuack-ng gen complete fish > tuack-ng.fish
            $out/bin/tuack-ng gen complete zsh > _tuack-ng

            installShellCompletion \
              --bash tuack-ng.bash \
              --fish tuack-ng.fish \
              --zsh _tuack-ng
          '';

          meta = with pkgs.lib; {
            description = "重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。";
            homepage = "https://github.com/tuack-ng/tuack-ng";
            license = licenses.agpl3Plus;
            platforms = platforms.unix;
            mainProgram = "tuack-ng";
          };
        };

        # tuack-ng-rpc：强制性依赖 tuack-ng，复用后者的资源产物
        # （assets / checkers / templates），不重复构建
        tuack-ng-rpc = craneLib.buildPackage {
          inherit src cargoArtifactsRpc;
          pname = "tuack-ng-rpc";
          inherit version;

          cargoExtraArgs = "-p tuack-ng-rpc --locked --no-default-features --features=nix";

          postInstall = ''
            mkdir -p $out/share
            ln -s ${tuack-ng}/share/tuack-ng $out/share/tuack-ng
          '';

          meta = with pkgs.lib; {
            description = "tuack-ng 竞赛工具后端协议服务（JSON-RPC 2.0 over stdio）";
            homepage = "https://github.com/tuack-ng/tuack-ng";
            license = licenses.agpl3Plus;
            platforms = platforms.unix;
            mainProgram = "tuack-ng-rpc";
          };
        };
      in
      {
        packages = {
          default = tuack-ng;
          tuack-ng = tuack-ng;
          tuack-ng-rpc = tuack-ng-rpc;
        };

        devShells.default = craneLib.devShell {
          inputsFrom = [ tuack-ng tuack-ng-rpc ];

          nativeBuildInputs = with pkgs; [
            rustToolchain
            rust-analyzer
            gcc
            typst
            testlib
          ];

          shellHook = ''
            export NIX_TESTLIB_PATH="${pkgs.testlib}/include/testlib/testlib.h"
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"

            echo 'Tuack-NG Dev Shell -- run `just --list` for information'
          '';
        };
      }
    );
}
