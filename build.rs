use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use vergen::{BuildBuilder, CargoBuilder, Emitter, RustcBuilder, SysinfoBuilder};

#[allow(unused)]
fn copy_testlib(source: PathBuf) -> io::Result<()> {
    let checkers_dir = "assets/checkers";
    let testlib_dest = format!("{}/testlib.h", checkers_dir);

    println!("cargo:rerun-if-changed={}", checkers_dir);
    println!("cargo:rerun-if-changed={}", source.display());

    // 确保目标目录存在
    fs::create_dir_all(checkers_dir)?;

    // 检查源文件是否存在
    if !source.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("testlib.h not found at {}", source.display()),
        ));
    }

    // 检查是否需要拷贝（文件不存在或内容不同）
    let should_copy = if Path::new(&testlib_dest).exists() {
        // 比较文件内容
        let src_content = fs::read(&source)?;
        let dst_content = fs::read(&testlib_dest)?;
        src_content != dst_content
    } else {
        true
    };

    if should_copy {
        fs::copy(&source, &testlib_dest)?;
    }
    Ok(())
}

fn main() {
    let checkers_dir = "assets/checkers";
    println!("cargo:rerun-if-changed={}", checkers_dir);
    #[cfg(not(feature = "nix"))]
    {
        copy_testlib("vendor/testlib/testlib.h".into()).unwrap();
    }
    #[cfg(feature = "nix")]
    {
        let testlib_path = std::env::var("NIX_TESTLIB_PATH").unwrap();
        copy_testlib(testlib_path.into()).unwrap();
    }
    // 编译 C++ 文件
    if let Ok(entries) = fs::read_dir(checkers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "cpp") {
                compile_cpp_if_needed(&path);
            }
        }
    }

    let build = BuildBuilder::default()
        .build_timestamp(true)
        .build()
        .unwrap();
    let cargo = CargoBuilder::default()
        .features(true)
        .debug(true)
        .opt_level(true)
        .target_triple(true)
        .dependencies(true)
        .build()
        .unwrap();
    let rustc = RustcBuilder::default()
        .semver(true)
        .channel(true)
        .build()
        .unwrap();
    let si = SysinfoBuilder::default().os_version(true).build().unwrap();

    Emitter::default()
        .add_instructions(&build)
        .unwrap()
        .add_instructions(&cargo)
        .unwrap()
        .add_instructions(&rustc)
        .unwrap()
        .add_instructions(&si)
        .unwrap()
        .quiet()
        .emit()
        .unwrap();
}

#[allow(unused)]
fn compile_cpp_if_needed(cpp_file: &Path) {
    let exe_name = cpp_file.with_extension(env::consts::EXE_EXTENSION);
    let exe_name = exe_name.file_name().unwrap().to_string_lossy();
    let exe_path = cpp_file.parent().unwrap().join(exe_name.to_string());

    // 检查可执行文件是否存在，以及是否比源码更旧
    let need_compile = if exe_path.exists() {
        // 获取文件修改时间
        let src_modified = fs::metadata(cpp_file)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let exe_modified = fs::metadata(&exe_path)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        // 如果源码比可执行文件新，则需要重新编译
        src_modified > exe_modified
    } else {
        // 可执行文件不存在，需要编译
        true
    };

    if need_compile {
        println!("cargo:warning=Compiling: {}", cpp_file.display());

        let mut cmd = Command::new("g++");
        cmd.current_dir(cpp_file.parent().unwrap())
            .arg("-std=c++17")
            .arg("-O2")
            .arg(cpp_file.file_name().unwrap())
            .arg("-o")
            .arg(exe_name.to_string());
        #[cfg(not(feature = "nix"))]
        {
            cmd.arg("-static"); // Nix 下 static 还是算了
        }

        let status = cmd.status();

        if let Ok(status) = status {
            if !status.success() {
                println!("cargo:warning=Failed to compile: {}", cpp_file.display());
            }
        } else {
            println!(
                "cargo:warning=Failed to execute g++ for: {}",
                cpp_file.display()
            );
        }
    }

    // 告诉 cargo 监听这个文件的变化
    println!("cargo:rerun-if-changed={}", cpp_file.display());
}
