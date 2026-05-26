# 🌱 Lumen

<p align="center">
  <strong>A next-gen lightning-fast, lightweight reference manager built with Rust & GPUI</strong>
</p>

<p align="center">
  <a href="#-core-features">Core Features</a> •
  <a href="#-why-lumen">Why Lumen</a> •
  <a href="#-download">Download</a> •
  <a href="#-roadmap--contributing">Roadmap</a>
</p>

---

Lumen was born from a simple pain point: academic software has become too bloated. Zotero is feature-rich but resource-heavy, and its UI grows sluggish over time. EndNote and other traditional tools have PDF reading experiences stuck in the last century.

In this new **vibe coding** era, we decided to build something truly good with a modern tech stack. Lumen rejects the bloat of Electron and other dynamic runtimes. It's built from the ground up with **Rust** and the **GPUI** framework (a high-performance GPU-accelerated UI framework by the Zed editor team), designed to deliver a **fast, lightweight, and modern** research experience.

---

## ✨ Core Features

### 📖 Native PDF Reader (GPU Hardware Accelerated)

- **Buttery smooth:** Full hardware acceleration for zooming, scrolling, and page-turning that far outperforms traditional PDF engines.
- **Deep interaction:** Cross-page text selection, powerful global text search, and high-quality highlighting and annotation.

### 📚 Literature & Citation Management

- **Smart metadata parsing:** Auto-fetch detailed metadata via DOI and other academic data sources — no manual entry needed.
- **Easy citation export:** Full CSL (Citation Style Language) and BibTeX (`.bib`) support, seamless integration with academic paper citations.

### 🌐 Multi-channel Cloud Sync

- **WebDAV support:** Easily connect your trusted cloud storage (Nutstore, InfiniCLOUD, etc.) for seamless PDF syncing.
- **Multi-device consistency:** MySQL-backed storage architecture ensures strong data consistency across devices — no more sync conflicts.

### 🌍 Smart Translation Engine

- **Immersive reading:** Built-in multi-language translation engine integrated into your research reading workflow. Current release prioritizes **English-to-Chinese** translation for barrier-free reading of international papers.

### 💻 Pure Native, Full Platform Coverage

No Electron baggage — pure native performance:

- **Windows:** Standard installer (`.exe`) and portable edition (`.zip`).
- **macOS (ARM64):** Native Apple Silicon support (*experimental*).
- **Linux:** Debian/Ubuntu package (`.deb`) (*experimental*).

---

## ⚡ Why Lumen?

| Feature | **Lumen** 🌱 | Zotero | EndNote |
| :--- | :--- | :--- | :--- |
| **Tech Stack** | **Rust + GPUI** | JavaScript + XULRunner | C++ / Legacy UI |
| **Memory & Resource Usage** | **Minimal (pure native, no virtual render layer)** | High (based on old Firefox engine) | Moderate |
| **PDF Rendering** | **Excellent (GPU hardware accelerated, smooth zooming)** | Sluggish (Web-based rendering) | Poor / Missing features |
| **Modern UI** | **Yes (Modern & Minimalist)** | Dated | Very Traditional |

---

## 🚀 Download

Head to the [Releases page](https://github.com/LumenLib/Lumen/releases) to download the latest version for your platform.

> ⚠️ **Note:** This project is currently in `0.1.0` pre-release. Core features are ready, but you may encounter rough edges due to differences in system environments and complex PDF formats. Early feedback is welcome!

---

## 🛠️ Building from Source

### Prerequisites

1. Install [Rust](https://www.rust-lang.org/) (Stable channel).
2. **Linux users:** Make sure X11 and xkbcommon libraries are installed (GPUI dependencies):

   ```bash
   sudo apt-get update && sudo apt-get install -y libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev
   ```

### Clone & Run

```bash
git clone https://github.com/LumenLib/Lumen.git
cd lumen
cargo run --release
```

---

## 🗺️ Roadmap

- [x] GPUI-based native GPU-accelerated PDF reader
- [x] Automatic metadata parsing via DOI
- [x] WebDAV file sync + MySQL database sync
- [x] Smart translation engine (English-to-Chinese priority)
- [ ] Cross-platform polish (macOS ARM64 & Linux)
- [ ] AI-powered literature Q&A and summaries

---

## 🤝 Contributing

All forms of contribution are welcome! Bug reports, feature suggestions, and pull requests are all appreciated.

If you encounter crashes or rough experiences on your specific setup, don't hesitate to open an Issue. Let's shape Lumen into the ultimate tool for researchers!

---

## 📄 License

This project is licensed under the MIT License.

Thanks for being part of Lumen's journey! If this project helps your research, give us a star 🌟!
