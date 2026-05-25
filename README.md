# Lumen

一个基于 [GPUI](https://github.com/zed-industries/zed) 框架的现代化科研文献管理工具。Rust 原生编写，GPU 加速渲染。

## 功能

### 已实现

- **文献管理**: 文献 CRUD、附件管理（自动重命名/拖拽导入/回收站删除）、多维标签、文件夹组织
- **PDF 阅读器**: 内置 PDFium 渲染，支持高亮/下划线/矩形框注释，注释跨设备同步
- **元数据抓取**: 自动从 DOI、ArXiv、DBLP、OpenAlex 解析元数据
- **引用分级**: CCF (A/B/C)、JCR (Q1-Q4) 自动标注
- **CSL 引文**: 完整集成 Citation Style Language，支持 10,000+ 期刊格式，一键拷贝 BibTeX/纯文本
- **云同步**: WebDAV 附件同步 + MySQL 元数据同步，冲突检测，按需下载
- **RSS 订阅**: 内置学术 RSS 追踪，支持订阅项导入文献库
- **文献查重**: 自动检测重复文献，支持字段级手动合并
- **BibTeX 导入/导出**: 批量导入导出
- **高级筛选**: 期刊/年份/作者/引用数等多维组合
- **国际化**: 9 种语言 (中/英/日/韩/法/德等)
- **笔记**: 文献内嵌 Markdown 笔记编辑器
- **自定义文件命名**: 可配置模板 (作者-年份-标题)
- **窗口状态记忆**: 自动保存/恢复窗口大小位置
- **文件变更监听**: 自动检测附件目录变更并触发同步
- **主题**: 亮色/暗色模式，自定义主题
- **跨平台**: macOS / Windows

### 计划中

- **Markdown 笔记**: 独立 Markdown 文档支持
- **多格式导入**: Mendeley / EndNote / Zotero / Paperpile
- **AI 摘要与对话**: 基于 LLM 的文献摘要和问答
- **浏览器扩展**: 一键抓取网页文献信息
- **插件系统**: 第三方功能扩展
- **Word 插件 / LaTeX 联动**: 写作工具集成
- **批量元数据补全**: 批量补全缺失的文献元数据
- **学术推荐**: 基于文献库的相关文献推荐

## 数据存储

| 类型 | 本地 | 远程 |
|------|------|------|
| 元数据 | SQLite (`lumen.db`) | MySQL |
| 文件 | `{attachment_path}/` | WebDAV |
| UI 状态 | SQLite (`state.db`) | — |
| 日志 | `{base_dir}/logs/` | — |

## 编译与运行

**依赖**: Rust 最新稳定版、macOS 或 Windows

```bash
git clone https://github.com/LumenLib/Lumen
cd Lumen
cargo run --release
```

**Windows 打包**: 使用 `build\win.ps1`（编译 + ZIP + 安装程序）

## 技术栈

| 类别 | 技术 |
|------|------|
| GUI 框架 | GPUI + gpui-component |
| 本地数据库 | SQLite |
| 远程数据库 | MySQL |
| 文件同步 | WebDAV |
| PDF 渲染 | PDFium |

## License

MIT


