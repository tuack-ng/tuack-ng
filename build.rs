use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let checkers_dir = "assets/checkers";
    println!("cargo:rerun-if-changed={}", checkers_dir);

    if !Path::new(checkers_dir).exists() {
        println!("cargo:warning=Directory not found");
        panic!();
    }

    if let Ok(entries) = fs::read_dir(checkers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "cpp") {
                compile_cpp_if_needed(&path);
            }
        }
    }
}

fn compile_cpp_if_needed(cpp_file: &Path) {
    let exe_name = cpp_file.with_extension("");
    let exe_name = exe_name.file_name().unwrap().to_string_lossy();

    // 获取输出文件路径
    let output_path = cpp_file.parent().unwrap().join(&*exe_name);

    // 检查是否需要重新编译
    if !needs_recompile(cpp_file, &output_path) {
        // println!("cargo:warning=Skipped: {} (up to date)", cpp_file.display());
        return;
    }

    compile_cpp(cpp_file, &output_path);
}

fn needs_recompile(source_file: &Path, output_file: &Path) -> bool {
    // 如果输出文件不存在，需要编译
    if !output_file.exists() {
        return true;
    }

    // 获取源文件和输出文件的修改时间
    let source_modified = fs::metadata(source_file)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let output_modified = fs::metadata(output_file)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);

    // 如果源文件比输出文件新，需要重新编译
    source_modified > output_modified
}

fn compile_cpp(cpp_file: &Path, output_path: &Path) {
    let exe_name = output_path.file_name().unwrap().to_string_lossy();

    let status = Command::new("g++")
        .current_dir(cpp_file.parent().unwrap())
        .arg("-std=c++17")
        .arg("-O2")
        .arg(cpp_file.file_name().unwrap())
        .arg("-o")
        .arg(&exe_name.to_string())
        .status();

    if let Ok(status) = status {
        if status.success() {
            // println!(
            //     "cargo:warning=Compiled: {} -> {}",
            //     cpp_file.display(),
            //     exe_name
            // );
        } else {
            println!("cargo:warning=Failed to compile: {}", cpp_file.display());
        }
    } else {
        println!("cargo:warning=Compiler error for: {}", cpp_file.display());
    }

    println!("cargo:rerun-if-changed={}", cpp_file.display());
}
