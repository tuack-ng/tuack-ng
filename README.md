<!--markdownlint-disable MD001 MD033 MD041 MD051-->

<div align="center">

# <image src="doc/assets/icon.svg" height="28" width="28"/> Tuack-NG

[![Stars](https://img.shields.io/github/stars/tuack-ng/tuack-ng?label=Stars)](https://github.com/tuack-ng/tuack-ng)
<!-- [![正式版 Release](https://img.shields.io/github/v/release/tuack-ng/tuack-ng?style=flat-square&color=%233fb950&label=正式版)](https://github.com/tuack-ng/tuack-ng/releases/latest) -->
[![测试版 Release](https://img.shields.io/github/v/release/tuack-ng/tuack-ng?include_prereleases&style=flat-square&label=测试版)](https://github.com/tuack-ng/tuack-ng/releases/)
[![下载量](https://img.shields.io/github/downloads/tuack-ng/tuack-ng/total?style=social&label=下载量&logo=github)](https://github.com/tuack-ng/tuack-ng/releases/latest)<br/>
![GitHub Repo size](https://img.shields.io/github/repo-size/tuack-ng/tuack-ng?style=flat-square&color=3cb371)
[![GitHub Repo Languages](https://img.shields.io/github/languages/top/tuack-ng/tuack-ng?style=flat-square)](https://github.com/tuack-ng/tuack-ng/search?l=c%23)

Tuack-NG（`Tuack New Generation`）是一套完整的用于辅助 OI/ACM 竞赛题目开发的套件，它的思想来自于 Tuack 项目。这个项目的目标是增强原 Tuack 的效率与易用性。

详见：[项目 / 计划：tuack-ng](https://pulsar33550336.github.io/2025/12/10/%E9%A1%B9%E7%9B%AE-%E8%AE%A1%E5%88%92%EF%BC%9Atuack-ng/)

<!-- #### 💬[Tuack-NG QQ 频道](https://pd.qq.com/s/grr6qwqwj) | [Tuack-NG QQ 群组](https://qm.qq.com/q/4NsDQKiAuQ) -->

#### [🌐 官方网站 ~~（没做）~~](https://tuack-ng.ink/) <!-- | [🚀 软件下载](https://tuack-ng.ink/download) -->｜[📚 项目文档](https://docs.tuack-ng.ink)<!-- ｜[🗳 功能投票](https://github.com/Tuack-NG/voting/discussions?discussions_q=is%3Aopen+sort%3Atop) -->

</div>

## 功能

### 生成题目工程 (`gen`)

- [x] 生成比赛/比赛日/题目 配置
- [ ] 生成与检测样例/预测试/数据
- [ ] 统一修改题目数据

### 渲染题目 (`ren`)

- [x] 渲染到 PDF（使用 Typst）
  - [x] NOI 格式
- [x] 渲染到 Markdown
- [ ] 渲染到 Html
- [ ] （...）
- [x] 基于 MiniJinja 的模板系统

### 测试题目 (`test`)

- [x] 测试 C++/C/Rust
- [x] 支持 Subtask

> [!TIP]
>
> 您可以点击下方链接或查看 [Tuack-NG 文档](https://docs.tuack-ng.ink) 了解更多。

## 软件截图

- 题目渲染

![1](https://pulsar33550336.github.io/img/1.svg)

- 测试

![2](https://pulsar33550336.github.io/img/2.svg)

## 开始使用

**首先，请确保您的设备满足以下推荐需求：**

- Debian（或其衍生版）或 Arch Linux。
- 对于其他系统及发行版的支持将在稍后添加。
  
> [!IMPORTANT]
> **详细安装说明请参阅 [Tuack-NG 文档](https://docs.tuack-ng.ink/guide/install)。**

对于普通用户，可以在以下渠道下载到本软件，请根据自身网络环境选择合适的渠道。

<!-- - [**Tuack-NG 官网（推荐）**](https://tuack-ng.ink/download) -->
- [GitHub Releases](https://github.com/tuack-ng/tuack-ng/releases/)
- [AUR（仅限 Arch Linux 可用）](https://aur.archlinux.org/packages/tuack-ng-bin)

## 获取帮助＆加入社区

您可以访问以下页面来**获取帮助**：

- [Tuack-NG 帮助文档](https://docs.tuack-ng.ink/guide/install)

您也可以加入这些社区**寻求帮助**：

[![GitHub Issues](https://img.shields.io/github/issues-search/tuack-ng/tuack-ng?query=is%3Aopen&style=flat-square&logo=github&label=Issues&color=%233fb950)](https://github.com/tuack-ng/tuack-ng/issues)
[![GitHub Discussions](https://img.shields.io/github/discussions/tuack-ng/tuack-ng?style=flat-square&logo=Github&label=Discussions)](https://github.com/tuack-ng/tuack-ng/discussions)
<!-- [![加入 QQ 频道](https://img.shields.io/badge/QQ_%E9%A2%91%E9%81%93-classisland-%230066cc?style=flat-square&logo=TencentQQ)](https://pd.qq.com/s/scb3wzia)
[![加入 QQ 群](https://img.shields.io/badge/QQ_%E7%BE%A4-958840932-%230066cc?style=flat-square&logo=TencentQQ)](https://qm.qq.com/q/4NsDQKiAuQ) -->

如果您确定您遇到的问题是一个 **Bug**，或者您要提出一项**新的功能**，请[提交 Issue](https://github.com/tuack-ng/tuack-ng/issues/new/choose)。

## 开发

![Alt](https://repobeats.axiom.co/api/embed/27c055ada1322dbf1ab82922ea66c615c194d36b.svg "Repobeats analytics image")

本项目目前开发状态：

| 分支 | 开发状态 | 状态 |
| :-: | :-: | :-: |
| `master` | 正在此分支上开发 [1.0 - Kaslana](https://github.com/tuack-ng/tuack-ng/milestone/1) | [![Build](https://github.com/tuack-ng/tuack-ng/actions/workflows/rust.yml/badge.svg?branch=master&style=flat_square)](<https://github.com/tuack-ng/tuack-ng/actions/workflows/rust.yml>) |

<!-- 要在本地编译应用，请参考文档[配置 Tuack-NG 本体开发环境](https://docs.tuack-ng.ink/dev/get-started/devlopment.html)。 -->

如果您有意愿为 Tuack-NG 做出代码贡献，请先阅读 ~~[贡献指南](CONTRIBUTING.md)~~ 没写 来了解如何为 Tuack-NG 做代码贡献。我们欢迎想要为本应用实现新功能或进行改进的同学提交 [Pull Request](https://github.com/tuack-ng/tuack-ng/pulls)。

您可以参考 [DeepWiki](https://deepwiki.com/tuack-ng/tuack-ng) 来了解项目结构。 [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/tuack-ng/tuack-ng)

## 致谢

<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-2-orange.svg?style=flat-square)](#contributors-)
<!-- ALL-CONTRIBUTORS-BADGE:END -->

感谢以下同学为本项目为本项目的开发提供支持（[✨](https://allcontributors.org/docs/zh-cn/emoji-key)）：

<!-- autocorrect-disable -->
<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/Cwhirly"><img src="https://avatars.githubusercontent.com/u/247487531?v=4?s=100" width="100px;" alt="Cwhirly"/><br /><sub><b>Cwhirly</b></sub></a><br /><a href="#design-Cwhirly" title="Design">🎨</a> <a href="https://github.com/tuack-ng/tuack-ng//tuack-ng/tuack-ng/commits?author=Cwhirly" title="Tests">⚠️</a> <a href="https://github.com/tuack-ng/tuack-ng//tuack-ng/tuack-ng/pulls?q=is%3Apr+reviewed-by%3ACwhirly" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/Pulsar33550336"><img src="https://avatars.githubusercontent.com/u/226428598?v=4?s=100" width="100px;" alt="Pulsar"/><br /><sub><b>Pulsar</b></sub></a><br /><a href="https://github.com/tuack-ng/tuack-ng//tuack-ng/tuack-ng/commits?author=Pulsar33550336" title="Code">💻</a> <a href="#ideas-Pulsar33550336" title="Ideas, Planning, & Feedback">🤔</a> <a href="https://github.com/tuack-ng/tuack-ng//tuack-ng/tuack-ng/commits?author=Pulsar33550336" title="Documentation">📖</a> <a href="#design-Pulsar33550336" title="Design">🎨</a> <a href="#maintenance-Pulsar33550336" title="Maintenance">🚧</a></td>
    </tr>
  </tbody>
  <tfoot>
    <tr>
      <td align="center" size="13px" colspan="7">
        <img src="https://raw.githubusercontent.com/all-contributors/all-contributors-cli/1b8533af435da9854653492b1327a23a4dbd0a10/assets/logo-small.svg">
          <a href="https://all-contributors.js.org/docs/en/bot/usage">Add your contributions</a>
        </img>
      </td>
    </tr>
  </tfoot>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->
<!-- autocorrect-enable -->
<!--markdownlint-disable MD001 MD033 MD041 MD051-->

## 许可证

本项目以 Affero General Public License 3.0 或更高版本获得许可。

## Stars 历史

[![Star 历史](https://starchart.cc/tuack-ng/tuack-ng.svg?variant=adaptive)](https://starchart.cc/tuack-ng/tuack-ng)

<div align="center">

如果这个项目对您有帮助，请点亮 Star ⭐

</div>
