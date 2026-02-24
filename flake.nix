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
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        version = cargoToml.package.version;

        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        lib = pkgs.lib;

        # 准备源代码
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.difference (lib.fileset.gitTracked ./.) (
            lib.fileset.fileFilter (file: file.name == "build.rs") ./.
          );
        };

        # 编译 checkers
        checkers =
          pkgs.runCommand "tuack-ng-checkers"
            {
              nativeBuildInputs = [
                pkgs.stdenv.cc
                pkgs.testlib
              ];
            }
            ''
              set -e

              mkdir -p $out/share/tuack-ng/checkers

              # 编译所有 cpp 文件
              for f in ${./assets/checkers}/*.cpp; do
                name=$(basename $f .cpp)
                $CXX -std=c++17 -O2 -I${pkgs.testlib}/include/testlib $f -o $out/share/tuack-ng/checkers/$name
                cp $f $out/share/tuack-ng/checkers/
              done
            '';

        # 准备 templates
        templates = pkgs.runCommand "tuack-ng-templates" { } ''
          mkdir -p $out/share/tuack-ng/templates
          cp -r ${templates-src}/* $out/share/tuack-ng/templates/
        '';

        assets = pkgs.symlinkJoin {
          name = "tuack-ng-assets";
          paths = [
            (pkgs.runCommand "tuack-ng-testlib" { } ''
              mkdir -p $out/share/tuack-ng/checkers
              ln -s ${pkgs.testlib}/include/testlib/testlib.h $out/share/tuack-ng/checkers/testlib.h
            '')
            checkers
            templates
            (pkgs.runCommand "tuack-ng-assets" { } ''
              mkdir -p $out/share/tuack-ng/
              cp -r ${src}/assets/* $out/share/tuack-ng/
            '')
          ];
        };

        # 构建依赖
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          pname = "tuack-ng";
          inherit version;
        };

        # 构建主程序
        tuack-ng = craneLib.buildPackage {
          inherit src cargoArtifacts;
          pname = "tuack-ng";
          inherit version;

          cargoExtraArgs = "--locked --no-default-features --features=nix";

          nativeBuildInputs = with pkgs; [
            installShellFiles
          ];

          # 使用 assets 作为构建依赖
          buildInputs = [ assets ];

          installPhase = ''
            runHook preInstall

            # 安装主程序
            mkdir -p $out/bin
            cp target/release/tuack-ng $out/bin/

            # 安装资产
            install -dm755 $out/share/
            # cp -r ${src}/assets/ $out/share/
            cp -r ${assets}/share/tuack-ng $out/share/ # 覆盖

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
          checkers = checkers;
          templates = templates;
          assets = assets;
        };

        devShells.default = craneLib.devShell {
          inputsFrom = [ tuack-ng ];

          nativeBuildInputs = with pkgs; [
            rustToolchain
            rust-analyzer
            gcc
            typst
          ];

          shellHook = ''
            export CHECKERS_PATH="${checkers}/share/tuack-ng/checkers"
            export TEMPLATES_PATH="${templates}/share/tuack-ng/templates"
            export TESTLIB_PATH="${pkgs.testlib}"
            export ASSETS_PATH="${assets}"
          '';
        };
      }
    );
}
