# {
#   description = "重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。";

#   inputs = {
#     nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
#     flake-utils.url = "github:numtide/flake-utils";
#     rust-overlay.url = "github:oxalica/rust-overlay";

#     crane = {
#       url = "github:ipetkov/crane";
#       # inputs.nixpkgs.follows = "nixpkgs";
#     };

#     # tuack-ng-src = {
#     #   url = "github:tuack-ng/tuack-ng?submodules=1";
#     #   flake = false;
#     # };
#     templates-src = {
#       url = "github:tuack-ng/templates";
#       flake = false;
#     };
#     testlib-src = {
#       url = "github:MikeMirzayanov/testlib";
#       flake = false;
#     };
#   };

#   outputs =
#     {
#       self,
#       nixpkgs,
#       flake-utils,
#       rust-overlay,
#       crane, # tuack-ng-src
#       templates-src,
#       testlib-src,
#     }:
#     flake-utils.lib.eachDefaultSystem (
#       system:
#       let
#         pkgs = import nixpkgs {
#           inherit system;
#           overlays = [ (import rust-overlay) ];
#         };

#         rustToolchain = pkgs.rust-bin.stable.latest.default;
#         craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

#         # 关键：完全模拟PKGBUILD的方式，使用git submodule！
#         # src = pkgs.runCommand "tuack-ng-with-submodules" {
#         #   src = tuack-ng-src;
#         #   templates = templates-src;
#         #   testlib = testlib-src;
#         #   nativeBuildInputs = [ pkgs.git ];  # 需要git命令
#         # } ''
#         # # 复制源代码并初始化git仓库
#         # cp -r $src $out
#         # chmod -R +w $out
#         # cd $out

#         # # 初始化git（为了submodule）
#         # git init
#         # git add .
#         # git commit -m "initial" > /dev/null 2>&1 || true

#         # 完全按照PKGBUILD的方式处理子模块！
#         # git submodule init
#         # git config submodule.assets/templates.url "$templates"
#         # git config submodule.vendor/testlib.url "$testlib"
#         # git -c protocol.file.allow=always submodule update

#         # 删除git历史节省空间
#         # rm -rf .git
#         # '';
#         # src = tuack-ng-src;
#         # src = pkgs.fetchgit {
#         #   url = "https://github.com/tuack-ng/tuack-ng.git";
#         #   rev = tuack-ng-src.rev;
#         #   sha256 = "sha256-pJqS0INCMMdRYPQosRSZZlA0WmPjtVH9d+ydBfl6h6s=";
#         #   fetchSubmodules = true;
#         # };
#         lib = pkgs.lib;
#         # src = lib.fileset.toSource {
#         #   root = ./.;
#         #   # fileset = lib.fileset.unions [
#         #   #   ./assets
#         #   #   # ./vendor
#         #   #   ./src
#         #   #   ./Cargo.toml
#         #   #   ./Cargo.lock
#         #   #   ./build.rs
#         #   #   (lib.fileset.gitTracked ./.)
#         #   # ];
#         #   fileset = lib.fileset.gitTracked ./.;
#         # };
#         src = pkgs.runCommand "prepared-source" { } ''
#           # 先创建所有需要的目录结构
#           mkdir -p $out/assets/templates
#           mkdir -p $out/vendor/testlib

#           # 然后复制主项目源到 $out
#           cp -r ${
#             lib.fileset.toSource {
#               root = ./.;
#               fileset = lib.fileset.gitTracked ./.;
#             }
#           }/* $out/

#           # 最后复制外部源到对应的子目录
#           cp -r ${templates-src}/* $out/assets/templates/
#           cp -r ${testlib-src}/* $out/vendor/testlib/
#         '';
#         # src = ./.;

#         # 构建所有依赖
#         cargoArtifacts = craneLib.buildDepsOnly {
#           inherit src;
#           pname = "tuack-ng";
#           version = "0.3.0";
#           # TEMPLATES_SRC = templates-src;
#           # TESTLIB_SRC = testlib-src;
#         };

#         # 构建主程序
#         tuack-ng = craneLib.buildPackage {
#           inherit src;
#           pname = "tuack-ng";
#           # version = "0.3.0-unstable-${builtins.substring 0 7 tuack-ng-src.rev or "0000000"}";
#           version = "0.3.0-unstable";
#           # TEMPLATES_SRC = templates-src;
#           # TESTLIB_SRC = testlib-src;
#           # OUT_PATH =

#           cargoArtifacts = cargoArtifacts;

#           # buildFeatures = ["static-checkers"];
#           cargoExtraArgs = "--locked --no-default-features";

#           nativeBuildInputs = with pkgs; [
#             installShellFiles
#             git # 构建时可能也需要git？
#           ];

#           postInstall = ''
#             # 安装资源文件（直接从src复制，src已经包含了子模块）
#             mkdir -p $out/share/tuack-ng
#             cp -r $src/assets/* $out/share/tuack-ng/
#             chmod -R u+w $out/share/tuack-ng

#             # 设置checkers可执行权限
#             find $out/share/tuack-ng/checkers -type f ! -name "*.*" -exec chmod 755 {} \;

#             # 生成补全
#             $out/bin/tuack-ng gen complete bash > tuack-ng.bash
#             $out/bin/tuack-ng gen complete fish > tuack-ng.fish
#             $out/bin/tuack-ng gen complete zsh > _tuack-ng

#             installShellCompletion \
#               --bash tuack-ng.bash \
#               --fish tuack-ng.fish \
#               --zsh _tuack-ng
#           '';

#           meta = with pkgs.lib; {
#             description = "重构后的 tuack 项目，旨在提供更加高效和轻量的出题体验。";
#             homepage = "https://github.com/tuack-ng/tuack-ng";
#             license = licenses.agpl3Only;
#             platforms = platforms.linux;
#             mainProgram = "tuack-ng";
#           };
#         };
#       in
#       {
#         packages.default = tuack-ng;
#         packages.tuack-ng = tuack-ng;

#         devShells.default = craneLib.devShell {
#           inputsFrom = [ tuack-ng ];

#           nativeBuildInputs = with pkgs; [
#             rustToolchain
#             rust-analyzer
#             typst
#             git-lfs
#           ];

#           shellHook = ''

#             export SRC_PATH="${src}"
#             echo "Source path: $SRC_PATH"
#           '';
#         };
#       }
#     );
# }

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
                    -I ${testlib-src} \
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
            install -m644 ${testlib-src}/testlib.h $out/share/tuack-ng/checkers/testlib.h

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
            export TESTLIB_PATH="${testlib-src}"
            echo "Checkers path: $CHECKERS_PATH"
            echo "Templates path: $TEMPLATES_PATH"
            echo "Testlib path: $TESTLIB_PATH"
          '';
        };
      }
    );
}
