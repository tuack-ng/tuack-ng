{
  description = "重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";

    crane = {
      url = "github:ipetkov/crane";
    };

    templates-src = {
      url = "github:tuack-ng/templates";
      flake = false;
    };
    testlib-src = {
      url = "github:MikeMirzayanov/testlib";
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
      testlib-src,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        lib = pkgs.lib;

        # 准备源代码（去除 build.rs）
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.difference (lib.fileset.gitTracked ./.) (
            lib.fileset.fileFilter (file: file.name == "build.rs") ./.
          );
        };

        # 编译 checkers
        checkers =
          pkgs.runCommand "build-checkers"
            {
              nativeBuildInputs = [ pkgs.gcc ];
            }
            ''
              mkdir -p $out/share/checkers

              # 编译所有 cpp 文件，直接使用 -I 指定 testlib.h 路径
              for cpp_file in ${./assets/checkers}/*.cpp; do
                if [ -f "$cpp_file" ]; then
                  filename=$(basename "$cpp_file")
                  exe_name="''${filename%.cpp}"

                  echo "Compiling $filename to $exe_name"
                  g++ -std=c++17 -O2 \
                    -I ${pkgs.testlib}/include/testlib \
                    "$cpp_file" \
                    -o "$out/share/checkers/$exe_name"

                  cp $cpp_file $out/share/checkers/

                  chmod +x "$out/share/checkers/$exe_name"
                fi
              done
            '';

        # 准备 templates
        templates = pkgs.runCommand "prepare-templates" { } ''
          mkdir -p $out/share/templates
          cp -r ${templates-src}/* $out/share/templates/
        '';

        # 构建依赖
        cargoArtifacts = craneLib.buildDepsOnly {
          inherit src;
          pname = "tuack-ng";
          version = "0.3.0";
        };

        # 构建主程序
        tuack-ng = craneLib.buildPackage {
          inherit src cargoArtifacts;
          pname = "tuack-ng";
          version = "0.3.0-unstable";

          cargoExtraArgs = "--locked --no-default-features";

          nativeBuildInputs = with pkgs; [
            installShellFiles
          ];

          postInstall = ''
            # 安装 checkers 可执行文件（755）
            install -d $out/share/tuack-ng/checkers
            install -m755 ${checkers}/share/checkers/* $out/share/tuack-ng/checkers/

            # 使用 find 安装所有 C++ 源文件（644）
            find ${./assets/checkers} -name "*.cpp" -type f -exec install -m644 {} $out/share/tuack-ng/checkers/ \;

            # 安装 testlib.h
            install -m644 ${pkgs.testlib}/include/testlib/testlib.h $out/share/tuack-ng/checkers/testlib.h

            # 安装 templates（使用 install 递归复制并设置权限）
            install -d $out/share/tuack-ng/templates
            cp -r ${templates}/share/templates/* $out/share/tuack-ng/templates/
            chmod -R u+w $out/share/tuack-ng/templates

            # 复制其他 assets（如果有的话，排除 checkers 和 templates）
            if [ -d "${src}/assets" ]; then
              for item in ${src}/assets/*; do
                if [ -d "$item" ]; then
                  dirname=$(basename "$item")
                  if [ "$dirname" != "checkers" ] && [ "$dirname" != "templates" ]; then
                    install -d $out/share/tuack-ng/"$dirname"
                    cp -r "$item"/* $out/share/tuack-ng/"$dirname"/
                    chmod -R u+w $out/share/tuack-ng/"$dirname"
                  fi
                fi
              done
            fi

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
            license = licenses.agpl3Only;
            platforms = platforms.linux;
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
            export CHECKERS_PATH="${checkers}/share/checkers"
            export TEMPLATES_PATH="${templates}/share/templates"
            export TESTLIB_PATH="${pkgs.testlib}"
            echo "Checkers path: $CHECKERS_PATH"
            echo "Templates path: $TEMPLATES_PATH"
            echo "Testlib path: $TESTLIB_PATH"
          '';
        };
      }
    );
}
