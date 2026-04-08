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
    /// 解包操作
    #[command(version)]
    Unwrap,
    /// 打包操作
    #[command(version)]
    Wrap,
}

fn sha256(src_path: &PathBuf) -> Result<String> {
    let mut file = std::fs::File::open(&src_path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);
    Ok(hash_hex)
}

pub fn main(args: DevelopArgs) -> Result<()> {
    match args.command {
        DevelopCommands::ShowConfig => show_config(),
        DevelopCommands::Unwrap => unwrap(),
        DevelopCommands::Wrap => wrap(),
    }
}

fn show_config() -> Result<()> {
    warn!("{:#?}", gctx().config);
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
    let templates_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("templates");
    let store_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("templates")
        .join("store");
    let unwrapped_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("templates")
        .join("unwrapped");

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
                    bail!("store 中找不到文件: {} (sha256: {})", relative_path, sha256);
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
    // todo!("wrap 命令尚未实现");
    // TODO: 实现 warp 逻辑
    let unwrapped_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("templates")
        .join("unwrapped");
    let templates_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("templates");
    let store_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("templates")
        .join("store");
    if !unwrapped_path.exists() {
        bail!("unwrapped 文件夹不存在: {:?}", unwrapped_path);
    }
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
                bail!("manifest 文件不存在: {:?}", manifest_path);
            }
            fs::copy(manifest_path, &templates_path.join(name.clone() + ".json"))?;

            let manifest_path = templates_path.join(name + ".json");

            let manifest_text = fs::read_to_string(&manifest_path)?;

            let mut manifest: TemplateManifest = serde_json::from_str(&manifest_text)?;

            let filelist = collect_files(&entry_path);

            for file in filelist {
                let sha256 = sha256(&file)?;
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
