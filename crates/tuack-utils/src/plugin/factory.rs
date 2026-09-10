//! 插件工厂：把已解析的模板/导出器 spec 实例化为 `Box<dyn ...>`。
//!
//! 模板文件作为一等能力，在实例化渲染器之前落入工作区 `tmp/tmp`（WASI `/tmp`，
//! 插件只读资源目录另见 `asset_dir` -> `/assets`）。

use tempfile::TempDir;

use tuack_lib::dump::Dumper;
use tuack_lib::ren::{RenProcessor, Renderer};

use crate::dump::extism::ExtismDumper;
use crate::plugin::manager::{
    BuiltinDumper, BuiltinRenderer, DumperRef, PluginManager, PluginRef, ProcessorRef, RendererRef,
    ResolvedTemplate, TemplateSource,
};
use crate::plugin::manifest::{ComponentBody, ComponentKind};
use crate::prelude::*;
use crate::ren::extism::ExtismRenderer;
use crate::ren::markdown::MarkdownRenderer;
use crate::ren::processors::builtin_processor;
use crate::ren::processors::extism::ExtismProcessor;
use crate::ren::renderers::unwrap_template;
use crate::ren::typst::TypstRenderer;

/// 落模板文件到工作区 `tmp/tmp`，再构造渲染器。
pub(crate) fn build_renderer(
    template: &ResolvedTemplate,
    registry: &PluginManager,
    tmp: Arc<TempDir>,
    assets_dirs: &[PathBuf],
) -> Result<Box<dyn Renderer>> {
    let template_dir = tmp.path().join("tmp");
    fs::create_dir_all(&template_dir)?;
    match &template.source {
        TemplateSource::Store { filelist } => {
            unwrap_template(filelist, &template_dir, assets_dirs)?;
        }
        TemplateSource::Dir(dir) => {
            copy_dir_contents(dir, &template_dir)?;
        }
        TemplateSource::Empty => {}
    }

    match &template.renderer {
        RendererRef::Builtin(BuiltinRenderer::Typst) => Ok(Box::new(TypstRenderer::new(tmp)?)),
        RendererRef::Builtin(BuiltinRenderer::Markdown) => Ok(Box::new(MarkdownRenderer::new())),
        RendererRef::Plugin(pref) => {
            let (bytes, func, asset_dir, command) = renderer_component(registry, pref)?;
            Ok(Box::new(ExtismRenderer::new(
                bytes, func, tmp, asset_dir, command,
            )?))
        }
    }
}

/// 构造处理器（内置或插件）。
pub(crate) fn build_processor(
    pref: &ProcessorRef,
    registry: &PluginManager,
) -> Result<Box<dyn RenProcessor>> {
    match pref {
        ProcessorRef::Builtin(name) => {
            builtin_processor(name).with_context(|| format!("无此处理器：{}", name))
        }
        ProcessorRef::Plugin(pref) => {
            let package = package(registry, pref)?;
            let component = component(
                &package.components,
                ComponentKind::Processor,
                &pref.component,
            )?;
            let ComponentBody::Processor(p) = &component.body else {
                bail!("组件 {} 不是 processor", pref.component);
            };
            let bytes = read_wasm(&entry_path(package)?)?;
            let asset_dir = package.asset_dir.as_ref().map(|d| package.dir.join(d));
            Ok(Box::new(ExtismProcessor::new(
                bytes,
                p.func.clone(),
                p.wasi,
                asset_dir,
                p.command.clone(),
            )?))
        }
    }
}

/// 构造导出器。
pub(crate) fn build_dumper(
    pref: &DumperRef,
    registry: &PluginManager,
    tmp: Arc<TempDir>,
    assets_dirs: &[PathBuf],
) -> Result<Box<dyn Dumper>> {
    match pref {
        DumperRef::Builtin(builtin) => Ok(match builtin {
            BuiltinDumper::Lemon => Box::new(crate::dump::lemon::LemonDumper::new(tmp)),
            BuiltinDumper::Arbiter => Box::new(crate::dump::arbiter::ArbiterDumper::new(
                tmp,
                assets_dirs.to_vec(),
            )),
            BuiltinDumper::CcrPlus => Box::new(crate::dump::ccr_plus::CcrPlusDumper::new(tmp)),
        }),
        DumperRef::Plugin(pref) => {
            let package = package(registry, pref)?;
            let component = component(&package.components, ComponentKind::Dumper, &pref.component)?;
            let ComponentBody::Dumper(d) = &component.body else {
                bail!("组件 {} 不是 dumper", pref.component);
            };
            let bytes = read_wasm(&entry_path(package)?)?;
            let asset_dir = package.asset_dir.as_ref().map(|d| package.dir.join(d));
            Ok(Box::new(ExtismDumper::new(
                bytes,
                d.func.clone(),
                tmp,
                asset_dir,
                d.command.clone(),
            )?))
        }
    }
}

/// renderer 组件解析结果：wasm 字节、函数名、资源目录、命令白名单。
type RendererParts = (Vec<u8>, String, Option<PathBuf>, Vec<String>);

/// 取插件 renderer 组件的 wasm 字节、函数名、资源目录与命令白名单。
fn renderer_component(registry: &PluginManager, pref: &PluginRef) -> Result<RendererParts> {
    let package = package(registry, pref)?;
    let component = component(
        &package.components,
        ComponentKind::Renderer,
        &pref.component,
    )?;
    let ComponentBody::Renderer(r) = &component.body else {
        bail!("组件 {} 不是 renderer", pref.component);
    };
    let bytes = read_wasm(&entry_path(package)?)?;
    let asset_dir = package.asset_dir.as_ref().map(|d| package.dir.join(d));
    Ok((bytes, r.func.clone(), asset_dir, r.command.clone()))
}

fn package<'a>(
    registry: &'a PluginManager,
    pref: &PluginRef,
) -> Result<&'a crate::plugin::manager::PluginPackage> {
    registry
        .package(&pref.package)
        .context(format!("未找到插件包：{}", pref.package))
}

fn component<'a>(
    components: &'a IndexMap<(ComponentKind, String), crate::plugin::manifest::Component>,
    kind: ComponentKind,
    name: &str,
) -> Result<&'a crate::plugin::manifest::Component> {
    components
        .get(&(kind, name.to_string()))
        .context(format!("未找到插件组件：{}", name))
}

/// 取插件的 wasm 入口宿主路径。
fn entry_path(package: &crate::plugin::manager::PluginPackage) -> Result<PathBuf> {
    let entry = package
        .entry
        .as_ref()
        .context("插件未声明 entry（wasm 入口）")?;
    Ok(package.dir.join(entry))
}

fn read_wasm(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("读取插件 wasm 失败：{}", path.display()))
}

/// 递归复制目录内容到 `dst`（不跟随符号链接）。
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_contents(&from, &to)?;
        } else if file_type.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)
                .with_context(|| format!("复制模板文件失败：{}", from.display()))?;
        } else {
            debug!("跳过模板目录中的非普通文件：{}", from.display());
        }
    }
    Ok(())
}
