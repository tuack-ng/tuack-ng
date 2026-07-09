use crate::prelude::*;
use crate::ren::manifest::TemplateManifest;
use crate::utils::filesystem::create_or_clear_dir;
use clap::{Args, Subcommand};
use sha2::Digest;
use sha2::Sha256;

#[derive(Args, Debug)]
#[command(version)]
pub struct DevelopArgs {
    #[command(subcommand)]
    pub command: DevelopCommands,
}

#[derive(Subcommand, Debug)]
#[command(version)]
pub enum DevelopCommands {
    /// 显示当前配置
    #[command(version)]
    ShowConfig,
    /// 打印诊断信息
    #[command(version)]
    Diagnostic,
    /// 解包操作
    #[command(version)]
    Unwrap,
    /// 打包操作
    #[command(version)]
    Wrap,
}

fn sha256(src_path: &PathBuf) -> Result<String> {
    let mut file = std::fs::File::open(src_path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    Ok(hash_hex)
}

pub fn main(args: DevelopArgs) -> Result<()> {
    match args.command {
        DevelopCommands::ShowConfig => show_config(),
        DevelopCommands::Diagnostic => diagnostic(),
        DevelopCommands::Unwrap => unwrap(),
        DevelopCommands::Wrap => wrap(),
    }
}

fn show_config() -> Result<()> {
    warn!("{:#?}", gctx().config);
    Ok(())
}

use sysinfo::System;

fn diagnostic() -> Result<()> {
    msg!("构建时间戳     : {}", env!("VERGEN_BUILD_TIMESTAMP"));
    msg!("构建特性开关   : {}", env!("VERGEN_CARGO_FEATURES"));
    msg!("是否为调试构建 : {}", env!("VERGEN_CARGO_DEBUG"));
    msg!("构建优化等级   : {}", env!("VERGEN_CARGO_OPT_LEVEL"));
    msg!("构建平台       : {}", env!("VERGEN_CARGO_TARGET_TRIPLE"));
    msg!("构建依赖       : {}", env!("VERGEN_CARGO_DEPENDENCIES"));
    msg!("Rustc 版本     : {}", env!("VERGEN_RUSTC_SEMVER"));
    msg!("Rustc 频道     : {}", env!("VERGEN_RUSTC_CHANNEL"));
    msg!("构建机系统版本 : {}", env!("VERGEN_SYSINFO_OS_VERSION"));

    let mut sys = System::new_all();
    sys.refresh_all();

    msg!("========== 运行时系统信息 ==========");

    // 系统名称和版本
    let os_name = System::name().unwrap_or_else(|| "未知".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "未知".to_string());
    let kernel = System::kernel_long_version();

    msg!("操作系统名称   : {}", os_name);
    msg!("操作系统版本   : {}", os_version);
    msg!("内核/版本      : {}", kernel);

    // 内存信息
    let total_memory = sys.total_memory();
    let total_memory_gb = total_memory as f64 / 1024.0 / 1024.0 / 1024.0;

    msg!(
        "总内存         : {:.2} GB ({})",
        total_memory_gb,
        total_memory
    );

    // CPU 信息
    let cpus = sys.cpus();
    let cpu_cores = cpus.len();
    let cpu_name = if let Some(first_cpu) = cpus.first() {
        first_cpu.brand()
    } else {
        "未知"
    };

    msg!("CPU 名称       : {}", cpu_name);
    msg!("CPU 核心数     : {} 个", cpu_cores);

    Ok(())
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // 递归子目录
                files.extend(collect_files(&path));
            } else if path.is_file() {
                // 排除特定文件
                if path.file_name() != Some(std::ffi::OsStr::new("manifest.json")) {
                    files.push(path);
                }
            }
        }
    }

    files
}

fn unwrap() -> Result<()> {
    let templates_path = std::env::current_dir()?;
    let unwrapped_path = templates_path.join("unwrapped");
    let store_path = templates_path.join("store");

    if !store_path.exists() {
        bail!("store 文件夹不存在：{:?}", store_path);
    }

    create_or_clear_dir(&unwrapped_path)?;

    for entry in fs::read_dir(&templates_path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() && entry_path.extension().and_then(|e| e.to_str()) == Some("json") {
            let manifest_name = entry_path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            msg_progress!("解包 {} 模板", manifest_name.bold());

            let manifest_text = fs::read_to_string(&entry_path)?;
            let mut manifest: TemplateManifest = serde_json::from_str(&manifest_text)?;

            let target_dir = unwrapped_path.join(&manifest_name);
            fs::create_dir_all(&target_dir)?;

            for (relative_path, sha256) in &manifest.filelist {
                let source_file = store_path.join(sha256);
                let target_file = target_dir.join(relative_path);

                if let Some(parent) = target_file.parent() {
                    fs::create_dir_all(parent)?;
                }

                if source_file.exists() {
                    fs::copy(&source_file, &target_file)?;
                } else {
                    bail!("store 中找不到文件：{} (名称：{})", relative_path, sha256);
                }
            }

            manifest.filelist = IndexMap::new();

            let target_manifest_path = target_dir.join("manifest.json");
            fs::write(
                &target_manifest_path,
                serde_json::to_string_pretty(&manifest)?,
            )?;
        }
    }

    Ok(())
}

fn wrap() -> Result<()> {
    let templates_path = std::env::current_dir()?;
    let unwrapped_path = templates_path.join("unwrapped");
    let store_path = templates_path.join("store");

    if !unwrapped_path.exists() {
        bail!("unwrapped 文件夹不存在：{:?}", unwrapped_path);
    }

    create_or_clear_dir(&store_path)?;
    for entry in fs::read_dir(&unwrapped_path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let name = entry_path
                .strip_prefix(&unwrapped_path)
                .unwrap()
                .display()
                .to_string();
            msg_progress!("包装 {} 文件夹", name.bold());
            let manifest_path = entry_path.join("manifest.json");
            if !manifest_path.exists() {
                bail!("manifest 文件不存在：{:?}", manifest_path);
            }
            fs::copy(manifest_path, templates_path.join(name.clone() + ".json"))?;

            let manifest_path = templates_path.join(name + ".json");

            let manifest_text = fs::read_to_string(&manifest_path)?;

            let mut manifest: TemplateManifest = serde_json::from_str(&manifest_text)?;

            let filelist = collect_files(&entry_path);

            for file in filelist {
                let sha256 = format!(
                    "{}{}",
                    &sha256(&file)?,
                    file.extension()
                        .map(|ext| format!(".{}", ext.to_string_lossy()))
                        .unwrap_or_default()
                );
                manifest.filelist.insert(
                    file.strip_prefix(&entry_path)?.display().to_string(),
                    sha256.clone(),
                );
                let obj_path = store_path.join(&sha256);
                if !obj_path.exists() {
                    fs::copy(file, obj_path)?;
                }
            }

            fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        }
    }

    Ok(())
}
