# JustFile for Tuack-NG

# 初始化项目所需信息
init:
    # Init git submodule
    git submodule update --init --recursive
    @if [ -f /etc/NIXOS ]; then \
        just _nixos-init; \
    fi

# 构建项目
build:
    @cargo build

# 以发布模式构建
build-release:
    @cargo build --release

# 运行
run +args="":
    @cargo run -- {{ args }}

# NixOS 底下 build.rs 编译 checkers 炸时使用的东西（笑）
# 但是使用 flake.nix 时不应该使用
[confirm("检测到 NixOS，是否运行 checkers 初始化？[y/N]")]
_nixos-init:
    #!/run/current-system/sw/bin/bash
    # 没错，这是（它宝贝的） NixOS 特色！
    set -e

    CHECKERS_DIR="assets/checkers"

    # 确保目录存在
    if [ ! -d "$CHECKERS_DIR" ]; then
        echo "$CHECKERS_DIR 不存在"
        exit 0
    fi

    # 遍历所有 .cpp 文件
    for cpp_file in "$CHECKERS_DIR"/*.cpp; do
        [ -f "$cpp_file" ] || continue

        # 生成可执行文件名
        exe_name="$(basename "$cpp_file" .cpp)"
        exe_path="$CHECKERS_DIR/$exe_name"

        # 检查是否需要编译
        need_compile=1
        if [ -f "$exe_path" ]; then
            # 比较修改时间
            if [ "$cpp_file" -nt "$exe_path" ]; then
                need_compile=1
            else
                need_compile=0
            fi
        fi

        if [ $need_compile -eq 1 ]; then
            echo "编译: $cpp_file"

            # 编译命令
            g++ -std=c++17 -O2 "$cpp_file" -o "$exe_path"

            if [ $? -eq 0 ]; then
                echo "编译成功: $exe_path"
            else
                echo "编译失败: $cpp_file"
                exit 1
            fi
        else
            echo "跳过: $cpp_file"
        fi
    done
