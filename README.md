# FastSP

FastSP 是一个基于 Tauri 的桌面语音转文字应用，使用 SenseVoice 模型提供高质量的实时语音识别。

## ✨ 特性

- 🎤 **多种触发模式**
  - 鼠标中键按住录音
  - Ctrl + Win 组合键按住录音
  - 右 Alt 键切换录音状态
  
- 🌍 **多语言支持**
  - 自动语言检测
  - 中文、英文、日语、韩语、粤语

- 🎯 **智能模型管理**
  - 按需下载模型（支持代理）
  - 支持量化版（~230MB）和非量化版（~820MB）
  - 实时切换模型版本
  - 支持从本地文件导入模型
  - 可取消下载任务

- 📝 **历史记录**
  - 自动保存转录历史
  - 一键复制文本
  - 清空历史功能

- 🤖 **LLM 纠错（可选）**
  - 支持 OpenAI 兼容 API
  - 自动修正语音识别错误（同音字、语法等）
  - 自定义纠错提示词
  - 支持代理配置
  - 连接测试功能

- 🎨 **视觉反馈**
  - 录音状态指示器（跟随鼠标）
  - LLM 处理状态指示（红色）
  - 音频输入测试

- 🔧 **灵活配置**
  - 自定义输入设备
  - 触发模式开关
  - 语言选择
  - 代理配置（HTTP/SOCKS5）
  - 模型文件夹管理

## 🚀 快速开始

### 环境要求

- Node.js 18+
- Rust 1.70+
- Windows 10/11

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建应用

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`

## 🎮 使用方法

1. **首次启动**：应用会提示下载 SenseVoice 模型
2. **选择触发方式**：在设置中启用你喜欢的触发模式
3. **配置音频设备**：在设置中选择你的麦克风设备（可选，默认使用系统默认设备）
4. **开始录音**：使用配置的快捷键或鼠标操作开始录音
   - 录音时会在鼠标附近显示青色指示器
   - LLM 处理时会显示红色指示器
5. **自动转录**：录音结束后自动转录并粘贴到光标位置
6. **查看历史**：在右侧历史面板查看所有转录记录

## 🛠️ 技术栈

### 前端
- React 19
- TypeScript
- Tailwind CSS v4
- Vite
- Lucide Icons

### 后端
- Rust
- Tauri v2
- sherpa-onnx (语音识别)
- cpal (音频捕获)
- rdev (全局输入监听)
- enigo (文本注入)
- reqwest (HTTP 客户端，支持代理)
- tokio (异步运行时)

## 📦 项目结构

```
fastsp/
├── src/                    # React 前端源码
│   ├── components/         # React 组件
│   ├── lib/               # API 封装
│   └── index.css          # 全局样式
├── src-tauri/             # Rust 后端源码
│   ├── src/
│   │   ├── lib.rs         # 主逻辑
│   │   ├── audio.rs       # 音频服务
│   │   ├── asr.rs         # 语音识别
│   │   ├── llm.rs         # LLM 纠错服务
│   │   ├── input_listener.rs  # 输入监听
│   │   ├── model_manager.rs   # 模型管理
│   │   ├── storage.rs     # 配置存储
│   │   └── http_client.rs # HTTP 客户端（代理支持）
│   └── tauri.conf.json    # Tauri 配置
└── package.json
```

## ⚙️ 配置文件

配置文件存储在：`%APPDATA%/com.fastsp/config.json`

```json
{
  "trigger_mouse": true,
  "trigger_hold": true,
  "trigger_toggle": true,
  "language": "",
  "model_version": "quantized",
  "model_dir": "C:\\Users\\...\\AppData\\Local\\com.fastsp\\models\\sense-voice",
  "input_device": "",
  "llm_config": {
    "enabled": false,
    "base_url": "https://api.openai.com/v1",
    "api_key": "",
    "model": "gpt-4o-mini",
    "custom_prompt": ""
  },
  "proxy": {
    "enabled": false,
    "url": ""
  }
}
```

### LLM 纠错配置

启用 LLM 纠错后，语音识别结果会发送到配置的 API 进行二次校正，修复常见的语音识别错误：

- **同音字/近音字错误**：如"他门" → "他们"
- **语法不通顺**：自动调整语序
- **标点符号问题**：补充或修正标点

支持任何 OpenAI 兼容的 API（如 OpenAI、DeepSeek、Ollama 等）。

### 代理配置

如果网络环境需要代理，可以在设置中配置：

- **HTTP 代理**：`http://127.0.0.1:7890`
- **SOCKS5 代理**：`socks5://127.0.0.1:1080`

代理配置会应用于：
- 模型下载
- LLM API 请求

### 模型管理

- **下载模型**：支持量化版和非量化版，可随时切换
- **导入模型**：支持从本地 `.tar.bz2` 文件导入模型
- **打开模型文件夹**：快速访问模型存储位置
- **取消下载**：支持取消正在进行的下载任务

### 音频测试

在设置中可以测试音频输入设备，实时查看音频输入电平，确保设备正常工作。

## 📝 开发说明

### 项目架构

- **状态管理**：使用 Tauri 的 State 管理服务状态，避免状态冗余
- **异步处理**：使用 Tokio 处理异步任务（下载、LLM 请求）
- **线程安全**：使用 `Arc<AtomicBool>` 防止并发处理冲突
- **错误处理**：使用 `anyhow` 和 `thiserror` 进行错误处理

### 添加新的触发模式

1. 在 `src-tauri/src/input_listener.rs` 添加监听逻辑
2. 在 `src-tauri/src/storage.rs` 的 `AppConfig` 添加配置项
3. 在 `src/components/SettingsModal.tsx` 添加 UI 控制

### 自定义主题色

在 `src/index.css` 的 `@theme` 块中修改：

```css
@theme {
    --color-chinese-indigo: #1661ab;
}
```

### 指示器窗口

应用使用独立的指示器窗口显示录音和 LLM 处理状态：
- 录音时：青色指示器（`#4f9d9a`）
- LLM 处理时：红色指示器（`#dc2626`）
- 指示器会跟随鼠标移动

### 数据存储位置

- **配置文件**：`%APPDATA%\com.fastsp\config.json`
- **历史记录**：`%APPDATA%\com.fastsp\history.json`
- **模型文件**：`%LOCALAPPDATA%\com.fastsp\models\sense-voice\`

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT License
