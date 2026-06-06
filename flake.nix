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
        version = cargoToml.package.version;

        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;
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
        };

        tuack-ng = craneLib.buildPackage {
          inherit src cargoArtifacts;
          pname = "tuack-ng";
          inherit version;

          cargoExtraArgs = "--locked --no-default-features --features=nix";

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

          installPhase = ''
            runHook preInstall

            mkdir -p $out/bin
            cp target/release/tuack-ng $out/bin/

            # 安装静态资源（build.rs 已处理好 testlib.h 和 checkers 编译）
            mkdir -p $out/share/tuack-ng
            cp -r assets/* $out/share/tuack-ng/

            # 我们使用系统的 testlib
            ln -sf ${pkgs.testlib}/include/testlib/testlib.h $out/share/tuack-ng/checkers/testlib.h

            # 安装 templates（来自独立 input，覆盖 assets/templates 的 gitlink）
            mkdir -p $out/share/tuack-ng/templates/
            cp -r ${templates-src}/* $out/share/tuack-ng/templates/
            chmod -R u+w $out/share/tuack-ng/templates/

            # 生成 shell 补全
            $out/bin/tuack-ng gen complete bash > tuack-ng.bash
            $out/bin/tuack-ng gen complete fish > tuack-ng.fish
            $out/bin/tuack-ng gen complete zsh > _tuack-ng

            installShellCompletion \
              --bash tuack-ng.bash \
              --fish tuack-ng.fish \
              --zsh _tuack-ng

            runHook postInstall
          '';

          meta = with pkgs.lib; {
            description = "重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。";
            homepage = "https://github.com/tuack-ng/tuack-ng";
            license = licenses.agpl3Only;
            platforms = platforms.unix;
            mainProgram = "tuack-ng";
          };
        };
      in
      {
        packages = {
          default = tuack-ng;
          tuack-ng = tuack-ng;
        };

        devShells.default = craneLib.devShell {
          inputsFrom = [ tuack-ng ];

          nativeBuildInputs = with pkgs; [
            rustToolchain
            rust-analyzer
            gcc
            typst
            testlib
          ];

          shellHook = ''
            export NIX_TESTLIB_PATH="${pkgs.testlib}/include/testlib/testlib.h"
          '';
        };
      }
    );
}
