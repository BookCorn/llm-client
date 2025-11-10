# LaTeX 公式集成方案

## 背景

在聊天应用中显示数学公式是一个常见需求，尤其是当 LLM 返回包含数学公式的内容时。LaTeX 是最流行的数学公式标记语言。

## 挑战

GPUI 是一个 GPU 加速的 UI 框架，目前没有内置的 LaTeX 渲染支持。需要选择合适的方案来渲染 LaTeX 公式。

---

## 方案对比

| 方案 | 渲染质量 | 性能 | 实现复杂度 | 离线支持 | 推荐度 |
|------|---------|------|-----------|---------|--------|
| 方案1: KaTeX Wasm | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ✅ | ⭐⭐⭐⭐⭐ |
| 方案2: 图片渲染API | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ❌ | ⭐⭐⭐⭐ |
| 方案3: MathML | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ | ✅ | ⭐⭐⭐ |
| 方案4: 自建LaTeX引擎 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐ | ✅ | ⭐⭐ |
| 方案5: SVG 渲染 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ✅ | ⭐⭐⭐⭐⭐ |

---

## 方案 1: KaTeX Wasm（推荐）⭐⭐⭐⭐⭐

### 简介

使用 KaTeX（快速的数学排版库）的 WebAssembly 版本在 Rust 中渲染 LaTeX，输出为 HTML/MathML。

### 优点

- ✅ **高质量渲染**：KaTeX 是业界标准，渲染质量优秀
- ✅ **快速**：比 MathJax 快得多
- ✅ **离线支持**：完全在本地运行
- ✅ **无需外部依赖**：通过 WASM 集成
- ✅ **丰富功能**：支持几乎所有 LaTeX 数学命令

### 缺点

- ⚠️ **需要 HTML 渲染**：输出是 HTML，需要在 GPUI 中渲染 HTML
- ⚠️ **包体积**：WASM 文件约 1-2MB

### 实现步骤

#### 1. 添加依赖

```toml
[dependencies]
# 选项 A: 使用 katex Rust crate（如果存在）
katex = "0.4"

# 选项 B: 或者直接使用 quickjs 来运行 KaTeX JS
quick-js = "0.4"
```

#### 2. 创建 LaTeX 渲染器

```rust
use katex;

pub struct LatexRenderer;

impl LatexRenderer {
    pub fn render_to_html(latex: &str) -> Result<String, String> {
        katex::render(latex)
            .map_err(|e| format!("LaTeX 渲染失败: {}", e))
    }

    pub fn render_inline(latex: &str) -> Result<String, String> {
        katex::render_with_opts(latex, katex::Opts::builder()
            .display_mode(false)
            .build()
            .unwrap())
            .map_err(|e| format!("LaTeX 渲染失败: {}", e))
    }
}
```

#### 3. 在 Markdown 中集成

扩展 Markdown 解析器以识别 LaTeX：

- 行内公式：`$...$` 或 `\\(...\\)`
- 块级公式：`$$...$$` 或 `\\[...\\]`

```rust
// 在 gpui-component 的 Markdown 解析器中添加 LaTeX 支持
// 或者预处理 Markdown 文本
fn preprocess_latex(markdown: &str) -> String {
    // 识别 $...$ 和 $$...$$ 模式
    // 调用 KaTeX 渲染为 HTML
    // 替换原始 LaTeX 为 HTML
}
```

#### 4. 示例

```rust
let latex = r"E = mc^2";
let html = LatexRenderer::render_inline(latex)?;
// 输出: <span class="katex">...</span>

let latex_block = r"\int_{-\infty}^{\infty} e^{-x^2} dx = \sqrt{\pi}";
let html = LatexRenderer::render_to_html(latex_block)?;
```

### 资源

- KaTeX: https://katex.org/
- katex-rs (Rust binding): https://github.com/xu-cheng/katex-rs

---

## 方案 2: 图片渲染 API（最简单）⭐⭐⭐⭐

### 简介

使用在线 API 将 LaTeX 渲染为图片（PNG/SVG），然后在 GPUI 中显示图片。

### 优点

- ✅ **实现最简单**：只需 HTTP 请求 + 图片显示
- ✅ **无需本地依赖**：所有渲染在服务器端完成
- ✅ **GPUI 原生支持**：GPUI 支持图片显示

### 缺点

- ❌ **需要网络**：离线无法使用
- ❌ **性能问题**：网络延迟，需要缓存
- ⚠️ **隐私问题**：公式内容发送到第三方服务器

### 可用服务

1. **QuickLaTeX** (推荐)
   - API: `https://quicklatex.com/latex3.f`
   - 免费，无需认证
   - 返回 PNG/SVG

2. **LaTeX.codecogs.com**
   - API: `https://latex.codecogs.com/png.latex?{latex}`
   - 直接返回图片 URL

3. **iTex2Img**
   - API: `http://www.sciweavers.org/tex2img.php?eq={latex}&bc=White&fc=Black&im=png`

### 实现步骤

#### 1. 创建 LaTeX 渲染服务

```rust
use reqwest;
use base64;

pub struct LatexImageRenderer {
    cache: HashMap<String, Vec<u8>>,  // LaTeX -> 图片数据
}

impl LatexImageRenderer {
    pub async fn render_to_png(&mut self, latex: &str) -> Result<Vec<u8>, Error> {
        // 检查缓存
        if let Some(cached) = self.cache.get(latex) {
            return Ok(cached.clone());
        }

        // 调用 API
        let url = format!("https://latex.codecogs.com/png.latex?{}",
            urlencoding::encode(latex));

        let response = reqwest::get(&url).await?;
        let bytes = response.bytes().await?.to_vec();

        // 缓存结果
        self.cache.insert(latex.to_string(), bytes.clone());

        Ok(bytes)
    }
}
```

#### 2. 在 UI 中显示

```rust
// 在 GPUI 中显示图片
use gpui::{img, SharedUri};

let latex = r"E = mc^2";
let png_data = renderer.render_to_png(latex).await?;
let uri = SharedUri::from_data("image/png", png_data);

div().child(
    img(uri)
        .object_fit(gpui::ObjectFit::Contain)
)
```

#### 3. Markdown 集成

```rust
fn process_latex_in_markdown(markdown: &str) -> String {
    // 正则匹配 $...$ 和 $$...$$
    let re = regex::Regex::new(r"\$\$([^$]+)\$\$|\$([^$]+)\$").unwrap();

    re.replace_all(markdown, |caps: &regex::Captures| {
        let latex = caps.get(1).or(caps.get(2)).unwrap().as_str();
        let img_url = format!("https://latex.codecogs.com/png.latex?{}",
            urlencoding::encode(latex));
        format!("![latex]({})", img_url)
    }).to_string()
}
```

### 示例

```rust
// 原始 Markdown
let text = "The famous equation is $E = mc^2$";

// 处理后
// "The famous equation is ![latex](https://latex.codecogs.com/png.latex?E%20%3D%20mc%5E2)"

// TextView 会自动渲染图片
TextView::markdown(id, &process_latex_in_markdown(text), window, cx)
```

---

## 方案 3: MathML（中等复杂度）⭐⭐⭐

### 简介

将 LaTeX 转换为 MathML（数学标记语言），然后在 GPUI 中渲染 MathML。

### 优点

- ✅ **标准化**：MathML 是 W3C 标准
- ✅ **离线**：完全本地处理
- ✅ **可访问性**：屏幕阅读器友好

### 缺点

- ⚠️ **渲染复杂**：需要实现 MathML 渲染器
- ⚠️ **有限支持**：并非所有浏览器都完全支持 MathML
- ❌ **GPUI 不原生支持**：需要自己实现渲染

### 实现步骤

需要：
1. LaTeX → MathML 转换器（如 `latex2mathml` crate）
2. MathML → GPUI 元素渲染器（需要自己实现）

**不推荐**，除非你需要特别的可访问性支持。

---

## 方案 4: 自建 LaTeX 引擎（最复杂）⭐⭐

### 简介

集成完整的 LaTeX 引擎（如 TeX Live），渲染为 PDF/DVI，然后转换为图片。

### 优点

- ✅ **完整支持**：支持所有 LaTeX 功能
- ✅ **最高质量**：真正的 TeX 渲染

### 缺点

- ❌ **体积巨大**：TeX Live 数百MB
- ❌ **性能差**：TeX 渲染很慢
- ❌ **实现极其复杂**

**不推荐**，仅用于特殊场景（如学术论文编辑器）。

---

## 方案 5: SVG 渲染（推荐）⭐⭐⭐⭐⭐

### 简介

使用 `mathjax-node` 或 `katex` 将 LaTeX 渲染为 SVG，然后在 GPUI 中显示 SVG。

### 优点

- ✅ **矢量图形**：无损缩放
- ✅ **体积小**：SVG 是文本格式
- ✅ **高质量**：完美渲染
- ✅ **GPUI 支持**：GPUI 支持 SVG（通过图片组件）

### 缺点

- ⚠️ **需要 SVG 渲染**：GPUI 的 SVG 支持可能有限

### 实现步骤

#### 1. 使用 KaTeX 生成 SVG

KaTeX 可以输出为 SVG：

```rust
let opts = katex::Opts::builder()
    .output_type(katex::OutputType::MathML)  // 或使用 SVG 输出
    .build()?;

let svg = katex::render_with_opts(latex, opts)?;
```

#### 2. 在 GPUI 中显示 SVG

```rust
use gpui::{img, SharedUri};

let svg_data = renderer.render_to_svg(latex).await?;
let uri = SharedUri::from_data("image/svg+xml", svg_data.into_bytes());

div().child(
    img(uri)
        .object_fit(gpui::ObjectFit::Contain)
)
```

---

## 推荐方案总结

### 短期快速方案：方案 2（图片渲染 API）

**适合场景**：
- 快速原型开发
- 不需要离线支持
- 公式数量不多

**实现步骤**：
1. 预处理 Markdown，识别 `$...$` 和 `$$...$$`
2. 将 LaTeX 转换为图片 URL
3. 替换为 Markdown 图片语法 `![latex](url)`
4. TextView 自动渲染图片

**代码量**：约 50-100 行

---

### 长期最佳方案：方案 5（SVG 渲染）+ 方案 1（KaTeX）

**适合场景**：
- 生产环境
- 需要离线支持
- 高质量渲染
- 频繁使用数学公式

**实现步骤**：
1. 集成 `katex-rs` crate
2. 扩展 gpui-component 的 Markdown 解析器，添加 LaTeX 节点类型
3. 渲染 LaTeX 节点为 SVG 图片
4. 缓存渲染结果

**代码量**：约 500-1000 行

---

## 实现优先级建议

### 第一阶段：基础支持（1-2天）

使用**方案 2（图片 API）**：

```rust
// 1. 添加 LaTeX 预处理函数
fn preprocess_latex(markdown: &str) -> String {
    let inline_re = Regex::new(r"\$([^$]+)\$").unwrap();
    let block_re = Regex::new(r"\$\$([^$]+)\$\$").unwrap();

    let text = block_re.replace_all(markdown, |caps: &Captures| {
        let latex = &caps[1];
        format!("![latex](https://latex.codecogs.com/png.latex?{})",
            urlencoding::encode(latex))
    });

    inline_re.replace_all(&text, |caps: &Captures| {
        let latex = &caps[1];
        format!("![latex](https://latex.codecogs.com/png.latex?\\inline%20{})",
            urlencoding::encode(latex))
    }).to_string()
}

// 2. 在渲染前处理
let processed = preprocess_latex(&message.content);
TextView::markdown(id, &processed, window, cx)
```

### 第二阶段：本地渲染（1-2周）

使用**方案 1 或 5（KaTeX/SVG）**：

1. 集成 `katex-rs`
2. 实现本地 SVG 生成
3. 实现缓存机制
4. 扩展 Markdown 解析器

### 第三阶段：优化（持续）

- 异步渲染，不阻塞 UI
- 智能缓存策略
- 渲染质量优化
- 支持自定义主题颜色

---

## 代码示例：快速原型

```rust
use regex::Regex;

pub fn render_latex_in_markdown(markdown: &str, use_api: bool) -> String {
    if use_api {
        render_with_api(markdown)
    } else {
        // 将来实现本地渲染
        markdown.to_string()
    }
}

fn render_with_api(markdown: &str) -> String {
    let block_re = Regex::new(r"\$\$\n?([\s\S]+?)\n?\$\$").unwrap();
    let inline_re = Regex::new(r"\$([^$\n]+)\$").unwrap();

    // 先处理块级公式
    let text = block_re.replace_all(markdown, |caps: &regex::Captures| {
        let latex = caps[1].trim();
        format!("\n\n![math](https://latex.codecogs.com/png.latex?\\dpi{{300}}{})\\n\\n",
            urlencoding::encode(latex))
    });

    // 再处理行内公式
    inline_re.replace_all(&text, |caps: &regex::Captures| {
        let latex = &caps[1];
        format!("![math](https://latex.codecogs.com/png.latex?\\inline%20{})",
            urlencoding::encode(latex))
    }).to_string()
}
```

使用：

```rust
let content = "Einstein's equation: $E=mc^2$

$$
\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}
$$";

let processed = render_latex_in_markdown(content, true);
TextView::markdown(id, &processed, window, cx)
```

---

## 总结

| 方案 | 实现难度 | 推荐度 | 使用场景 |
|------|---------|--------|---------|
| 图片 API | ⭐ 简单 | ⭐⭐⭐⭐ | 快速原型，联网环境 |
| KaTeX Wasm | ⭐⭐⭐ 中等 | ⭐⭐⭐⭐⭐ | 生产环境，离线支持 |
| SVG 渲染 | ⭐⭐⭐ 中等 | ⭐⭐⭐⭐⭐ | 高质量需求 |
| MathML | ⭐⭐⭐⭐ 困难 | ⭐⭐ | 可访问性优先 |
| 自建引擎 | ⭐⭐⭐⭐⭐ 极难 | ⭐ | 学术软件 |

**我的建议**：
1. **现在**：使用方案 2（图片 API）快速实现
2. **之后**：迁移到方案 1 或 5（KaTeX/SVG）实现离线和高质量渲染

---

**最后更新**: 2025-01-05
**状态**: 提案阶段
