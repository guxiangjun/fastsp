# LLM 在线纠错功能实现计划

## 功能概述

在语音识别完成后，可选地通过 LLM（OpenAI 兼容接口）对识别结果进行纠错，然后输出纠正后的文本。

## 架构设计

### 配置项（存储在 AppConfig 中）

```rust
pub struct LlmConfig {
    pub enabled: bool,           // 是否启用 LLM 纠错
    pub base_url: String,        // API Base URL (如 https://api.openai.com/v1)
    pub api_key: String,         // API Key
    pub model: String,           // 模型名称 (如 gpt-4o-mini)
    pub custom_prompt: String,   // 自定义 prompt（空则使用默认）
}
```

### 默认 Prompt（用于语音纠错）

```
你是一个语音识别纠错助手。用户会提供语音识别的原始文本，其中可能包含：
- 同音字/近音字错误
- 语法不通顺
- 漏字或多字
- 标点符号问题

请修正这些错误，保持原意不变。

重要规则：
1. 只修正明显的错误，不要改变语义或风格
2. 如果原文已经正确，直接返回原文
3. 必须以 JSON 格式返回结果

输入文本：{text}

请以如下 JSON 格式返回（不要包含其他内容）：
{"corrected": "纠正后的文本"}
```

### 后端实现

#### 1. 新增 `src-tauri/src/llm.rs` - LLM 服务模块

```rust
// 主要功能：
// - correct_text(text: &str, config: &LlmConfig) -> Result<String>
// - 调用 OpenAI 兼容 API（/v1/chat/completions）
// - 解析 JSON 响应，提取 corrected 字段
```

#### 2. 修改 `src-tauri/src/storage.rs`

- 在 `AppConfig` 中添加 `llm_config: LlmConfig` 字段
- 添加默认值

#### 3. 修改 `src-tauri/src/lib.rs`

- 在转录完成后的流程中集成 LLM 纠错
- 流程变为：录音 -> ASR 转录 -> (若启用) LLM 纠错 -> 粘贴文本
- 添加新的 Tauri 命令用于测试 LLM 连接

### 前端实现

#### 1. 修改 `src/lib/api.ts`

- 添加 `LlmConfig` 类型
- 更新 `AppConfig` 类型
- 添加 `testLlmConnection` API

#### 2. 修改 `src/components/SettingsModal.tsx`

- 新增 "LLM 纠错" 配置区域：
  - 启用/禁用开关（使用现有的 TriggerToggle 样式）
  - Base URL 输入框
  - API Key 输入框（密码类型）
  - Model 名称输入框
  - 自定义 Prompt 文本域（可折叠）
  - 测试连接按钮

#### 3. 修改 `src/components/StatusSection.tsx`

- 在主页状态区域显示 LLM 纠错状态（可选，如显示小图标）

## 数据流

```
录音停止
    ↓
ASR 转录 (asr.transcribe)
    ↓
检查 config.llm_config.enabled
    ↓ (如果启用)
调用 LLM 纠错 (llm.correct_text)
    ↓
解析 JSON 响应
    ↓
保存历史记录
    ↓
粘贴文本
```

## 实现步骤

### Phase 1: 后端基础设施
1. 在 `storage.rs` 中添加 `LlmConfig` 结构体和默认值
2. 创建 `llm.rs` 模块，实现 OpenAI API 调用
3. 在 `lib.rs` 中注册 LLM 服务状态

### Phase 2: 后端集成
4. 修改转录流程，在 ASR 后添加 LLM 纠错步骤
5. 添加 `test_llm_connection` Tauri 命令

### Phase 3: 前端配置界面
6. 更新 `api.ts` 添加类型和 API
7. 在 `SettingsModal.tsx` 添加 LLM 配置区域

### Phase 4: 测试和优化
8. 端到端测试
9. 错误处理优化

## 依赖项

后端无需新增 crate，现有的 `reqwest` 和 `serde_json` 已足够。

## 注意事项

1. API Key 安全：存储在本地配置文件中，UI 显示为密码输入框
2. 错误处理：LLM 调用失败时应回退到原始 ASR 结果，不阻塞正常使用
3. 超时控制：LLM 调用应设置合理超时（如 10 秒）
4. JSON 解析：使用稳健的解析逻辑，处理 LLM 可能返回的格式变体
