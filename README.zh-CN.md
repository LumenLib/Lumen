# 🌱 Lumen

<p align="center">
  <strong>基于 Rust & GPUI 构建的下一代极速、轻量级文献管理器</strong>
</p>

<p align="center">
  <a href="#-核心特性">核心特性</a> •
  <a href="#-为什么选择-lumen">为什么选择 Lumen</a> •
  <a href="#-下载安装">下载安装</a> •
  <a href="#-路线图--参与贡献">路线图</a>
</p>

---

Lumen 的诞生源于一个极其简单的痛点：常用的学术软件太臃肿了。Zotero 功能虽完善，但资源占用极高，界面日渐迟钝；EndNote 等传统工具的 PDF 阅读体验更是停留在上个世纪。

正逢 **vibe coding** 的大航海时代，我们决定用现代技术栈打造一款真正好用的产品。Lumen 拒绝了 Electron 等动态运行时的臃肿包袱，完全基于 **Rust** 语言与 **GPUI** 框架（由 Zed 编辑器团队开发的高性能全 GPU 加速 UI 框架）从底层构建，旨在实现**极速、轻量与现代化**的科研体验。

---

## ✨ 核心特性

### 📖 原生 PDF 阅读器（GPU 硬件加速）

- **丝滑体验：** 完美调用底层硬件加速，带来远超传统 PDF 引擎的缩放、滚动与翻页性能。
- **深度交互：** 支持跨页文本选择、全局强力文本搜索以及高质量的高亮、标注功能。

### 📚 文献与引用管理

- **元数据智能解析：** 支持通过 DOI 等多种学术数据源，自动抓取并解析文献的详细元数据，无需手动录入。
- **引文便捷导出：** 完全支持 CSL（引文格式语言）与 BibTeX（`.bib`），无缝对接各类学术论文的参考文献引用。

### 🌐 全渠道云同步

- **WebDAV 支持：** 轻松绑定你信任的网盘（如坚果云、InfiniCLOUD 等），无缝同步文献 PDF 文件。
- **多设备多端一致：** 集成 MySQL 存储架构，确保多设备之间的数据强一致性，告别同步冲突。

### 🌍 智能翻译引擎

- **沉浸式阅读：** 内置多语言翻译引擎，完美融入科研阅读工作流。当前版本优先支持**英译中**，助你无障碍流畅阅读国际文献。

### 💻 纯原生，全平台覆盖

没有任何内嵌浏览器（Electron）的包袱，享受纯粹的原生性能：

- **Windows：** 提供标准安装程序（`.exe`）与免安装便携版（`.zip`）。
- **macOS（ARM64）：** 原生支持 Apple Silicon 芯片（*实验性支持*）。
- **Linux：** 提供 Debian/Ubuntu 软件包（`.deb`）（*实验性支持*）。

---

## ⚡ 为什么选择 Lumen？

| 特性 | **Lumen** 🌱 | Zotero | EndNote |
| :--- | :--- | :--- | :--- |
| **核心技术栈** | **Rust + GPUI** | JavaScript + XULRunner | C++ / 传统 UI |
| **内存与资源占用** | **极低（纯原生，无虚拟渲染层）** | 极高（基于旧版火狐引擎） | 中等 |
| **PDF 渲染性能** | **极佳（GPU 硬件加速，丝滑缩放）** | 较卡顿（依赖 Web 渲染） | 体验较差 / 功能缺失 |
| **界面现代化** | **是（Modern & Minimalist）** | 较老旧 | 非常传统 |

---

## 🚀 下载安装

请前往 [Releases 页面](https://github.com/LumenLib/Lumen/releases) 下载对应系统的最新版本。

> ⚠️ **注意：** 目前项目处于 `0.1.0` 预发布（Pre-release）阶段。核心功能已就绪，但由于不同系统环境和复杂 PDF 格式的差异，可能会遇到一些小瑕疵（Rough edges）。欢迎早期体验者向我们反馈！

---

## 🛠️ 本地开发与构建

如果你想从源码构建或为 Lumen 贡献代码：

### 前置要求

1. 安装 [Rust](https://www.rust-lang.org/)（Stable 渠道）。
2. **Linux 用户：** 确保系统已安装 X11 和 xkbcommon 库（GPUI 开发依赖）：

   ```bash
   sudo apt-get update && sudo apt-get install -y libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev
   ```

### 克隆与运行

```bash
git clone https://github.com/LumenLib/Lumen.git
cd lumen
cargo run --release
```

---

## 🗺️ 路线图

- [x] 基于 GPUI 的本地 GPU 加速 PDF 阅读器
- [x] 通过 DOI 自动解析文献元数据
- [x] WebDAV 文件同步与 MySQL 数据库同步
- [x] 智能翻译引擎（优先支持英译中）
- [ ] 跨平台支持（完善 macOS ARM64 与 Linux 版本）
- [ ] AI 驱动的智能文献问答与总结


---

## 🤝 参与贡献

我们极其欢迎任何形式的贡献！无论是提交 Bug（Issue）、提出新功能设想，还是直接提交代码（Pull Request）。

如果你在特定配置下发生崩溃或遇到不顺畅的体验，请毫不犹豫地提交 Issue。让我们一起把 Lumen 雕琢成科研人员的终极利器！

---

## 📄 开源协议

本项目采用 MIT License 开源协议。

感谢你见证并参与 Lumen 的起点！如果这个项目对你的科研有所帮助，欢迎点一个星星 🌟 支持我们！
