use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let checkers_dir = "assets/checkers";
    println!("cargo:rerun-if-changed={}", checkers_dir);

    // 确保目录存在
    fs::create_dir_all(checkers_dir).ok();

    // 下载 testlib.h
    let testlib_path = format!("{}/testlib.h", checkers_dir);
    if !Path::new(&testlib_path).exists() {
        download_file(
            "https://raw.githubusercontent.com/MikeMirzayanov/testlib/master/testlib.h",
            &testlib_path,
        );
    }

    // 编译 C++ 文件
    if let Ok(entries) = fs::read_dir(checkers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "cpp") {
                compile_cpp(&path);
            }
        }
    }
}

fn download_file(url: &str, output_path: &str) {
    match ureq::get(url).call() {
        Ok(mut response) => {
            if response.status() == 200 {
                if let Ok(content) = response.body_mut().read_to_string() {
                    if !fs::write(output_path, content).is_ok() {
                        println!("cargo:warning=Failed to write: {}", output_path);
                    }
                }
            }
        }
        Err(e) => {
            println!("cargo:warning=Failed to download {}: {}", url, e);
        }
    }

    if Path::new(output_path).exists() {
        println!("cargo:rerun-if-changed={}", output_path);
    }
}

fn compile_cpp(cpp_file: &Path) {
    let exe_name = cpp_file.with_extension("");
    let exe_name = exe_name.file_name().unwrap().to_string_lossy();

    let status = Command::new("g++")
        .current_dir(cpp_file.parent().unwrap())
        .arg("-std=c++17")
        .arg("-O2")
        .arg(cpp_file.file_name().unwrap())
        .arg("-o")
        .arg(&exe_name.to_string())
        .status();

    if let Ok(status) = status {
        if !status.success() {
            println!("cargo:warning=Failed to compile: {}", cpp_file.display());
        }
    }

    println!("cargo:rerun-if-changed={}", cpp_file.display());
}
