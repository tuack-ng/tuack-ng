use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

fn copy_testlib() -> io::Result<()> {
    let checkers_dir = "assets/checkers";
    let testlib_source = "vendor/testlib/testlib.h";
    let testlib_dest = format!("{}/testlib.h", checkers_dir);

    println!("cargo:rerun-if-changed={}", checkers_dir);
    println!("cargo:rerun-if-changed={}", testlib_source);

    // 确保目标目录存在
    fs::create_dir_all(checkers_dir)?;

    // 检查源文件是否存在
    if !Path::new(testlib_source).exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("testlib.h not found at {}", testlib_source),
        ));
    }

    // 检查是否需要拷贝（文件不存在或内容不同）
    let should_copy = if Path::new(&testlib_dest).exists() {
        // 比较文件内容
        let src_content = fs::read(testlib_source)?;
        let dst_content = fs::read(&testlib_dest)?;
        src_content != dst_content
    } else {
        true
    };

    if should_copy {
        fs::copy(testlib_source, &testlib_dest)?;
    }
    Ok(())
}

fn main() {
    let checkers_dir = "assets/checkers";
    println!("cargo:rerun-if-changed={}", checkers_dir);

    copy_testlib().unwrap();

    // 编译 C++ 文件
    if let Ok(entries) = fs::read_dir(checkers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "cpp") {
                compile_cpp_if_needed(&path);
            }
        }
    }
}

fn compile_cpp_if_needed(cpp_file: &Path) {
    let exe_name = cpp_file.with_extension("");
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

        let status = Command::new("g++")
            .current_dir(cpp_file.parent().unwrap())
            .arg("-std=c++17")
            .arg("-O2")
            .arg(cpp_file.file_name().unwrap())
            .arg("-o")
            .arg(exe_name.to_string())
            .arg("-static")
            .status();

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
