# JustFile for Tuack-NG

# 初始化项目所需信息
init:
    # Init git submodule
    git submodule update --init --recursive

# 构建项目
build:
    @cargo build

# 以发布模式构建
build-release:
    @cargo build --release

# 运行
run +args="":
    @cargo run -- {{ args }}
