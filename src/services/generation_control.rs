/// 文本生成控制
///
/// 提供对模型文本生成过程的精细控制，支持 GPT-5 等高级模型的参数
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 文本生成预设
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GenerationPreset {
    /// 精确模式 - 确定性高、温度低
    Precise,

    /// 平衡模式 - 默认设置
    Balanced,

    /// 创意模式 - 温度高、更随机
    Creative,

    /// 简洁模式 - 倾向于简短回答
    Concise,

    /// 详细模式 - 倾向于详细回答
    Detailed,

    /// 自定义模式
    Custom(GenerationParams),
}

impl GenerationPreset {
    /// 转换为生成参数
    pub fn to_params(&self) -> GenerationParams {
        match self {
            GenerationPreset::Precise => GenerationParams {
                temperature: Some(0.3),
                top_p: Some(0.9),
                frequency_penalty: Some(0.0),
                presence_penalty: Some(0.0),
                max_tokens: None,
                stop_sequences: Vec::new(),
                ..Default::default()
            },
            GenerationPreset::Balanced => GenerationParams::default(),
            GenerationPreset::Creative => GenerationParams {
                temperature: Some(0.9),
                top_p: Some(0.95),
                frequency_penalty: Some(0.5),
                presence_penalty: Some(0.3),
                max_tokens: None,
                stop_sequences: Vec::new(),
                ..Default::default()
            },
            GenerationPreset::Concise => GenerationParams {
                temperature: Some(0.5),
                top_p: Some(0.9),
                frequency_penalty: Some(0.2),
                presence_penalty: Some(0.0),
                max_tokens: Some(512),
                stop_sequences: Vec::new(),
                ..Default::default()
            },
            GenerationPreset::Detailed => GenerationParams {
                temperature: Some(0.7),
                top_p: Some(0.9),
                frequency_penalty: Some(0.0),
                presence_penalty: Some(0.3),
                max_tokens: Some(4096),
                stop_sequences: Vec::new(),
                ..Default::default()
            },
            GenerationPreset::Custom(params) => params.clone(),
        }
    }
}

/// 文本生成参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationParams {
    /// 温度 (0.0 - 2.0)
    /// 控制随机性，值越高越随机
    pub temperature: Option<f64>,

    /// Top-P 采样 (0.0 - 1.0)
    /// 核采样，控制候选 token 的累积概率
    pub top_p: Option<f64>,

    /// 频率惩罚 (-2.0 - 2.0)
    /// 降低重复使用相同 token 的可能性
    pub frequency_penalty: Option<f64>,

    /// 存在惩罚 (-2.0 - 2.0)
    /// 增加谈论新主题的可能性
    pub presence_penalty: Option<f64>,

    /// 最大 token 数
    pub max_tokens: Option<u32>,

    /// 停止序列
    pub stop_sequences: Vec<String>,

    /// Top-K 采样 (实验性)
    pub top_k: Option<u32>,

    /// 重复惩罚 (实验性)
    pub repetition_penalty: Option<f64>,

    /// 长度惩罚 (实验性)
    pub length_penalty: Option<f64>,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            top_p: Some(1.0),
            frequency_penalty: Some(0.0),
            presence_penalty: Some(0.0),
            max_tokens: None,
            stop_sequences: Vec::new(),
            top_k: None,
            repetition_penalty: None,
            length_penalty: None,
        }
    }
}

impl GenerationParams {
    /// 应用到 JSON 请求体
    pub fn apply_to_json(&self, json: &mut Value) {
        if let Some(temp) = self.temperature {
            json["temperature"] = serde_json::json!(temp);
        }

        if let Some(top_p) = self.top_p {
            json["top_p"] = serde_json::json!(top_p);
        }

        if let Some(freq) = self.frequency_penalty {
            json["frequency_penalty"] = serde_json::json!(freq);
        }

        if let Some(pres) = self.presence_penalty {
            json["presence_penalty"] = serde_json::json!(pres);
        }

        if let Some(max_tokens) = self.max_tokens {
            json["max_tokens"] = serde_json::json!(max_tokens);
        }

        if !self.stop_sequences.is_empty() {
            json["stop"] = serde_json::json!(self.stop_sequences);
        }

        // 实验性参数（可能不被所有模型支持）
        if let Some(top_k) = self.top_k {
            json["top_k"] = serde_json::json!(top_k);
        }

        if let Some(rep) = self.repetition_penalty {
            json["repetition_penalty"] = serde_json::json!(rep);
        }

        if let Some(len) = self.length_penalty {
            json["length_penalty"] = serde_json::json!(len);
        }
    }

    /// 验证参数有效性
    pub fn validate(&self) -> Result<(), String> {
        if let Some(temp) = self.temperature {
            if !(0.0..=2.0).contains(&temp) {
                return Err(format!("温度必须在 0.0-2.0 之间，当前: {}", temp));
            }
        }

        if let Some(top_p) = self.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                return Err(format!("top_p 必须在 0.0-1.0 之间，当前: {}", top_p));
            }
        }

        if let Some(freq) = self.frequency_penalty {
            if !(-2.0..=2.0).contains(&freq) {
                return Err(format!("频率惩罚必须在 -2.0-2.0 之间，当前: {}", freq));
            }
        }

        if let Some(pres) = self.presence_penalty {
            if !(-2.0..=2.0).contains(&pres) {
                return Err(format!("存在惩罚必须在 -2.0-2.0 之间，当前: {}", pres));
            }
        }

        Ok(())
    }

    /// 合并参数（self 优先）
    pub fn merge(&self, other: &GenerationParams) -> GenerationParams {
        GenerationParams {
            temperature: self.temperature.or(other.temperature),
            top_p: self.top_p.or(other.top_p),
            frequency_penalty: self.frequency_penalty.or(other.frequency_penalty),
            presence_penalty: self.presence_penalty.or(other.presence_penalty),
            max_tokens: self.max_tokens.or(other.max_tokens),
            stop_sequences: if self.stop_sequences.is_empty() {
                other.stop_sequences.clone()
            } else {
                self.stop_sequences.clone()
            },
            top_k: self.top_k.or(other.top_k),
            repetition_penalty: self.repetition_penalty.or(other.repetition_penalty),
            length_penalty: self.length_penalty.or(other.length_penalty),
        }
    }
}

/// 生成指导 - 用于引导模型生成特定风格的文本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationGuidance {
    /// 系统级指导（高优先级）
    pub system_guidance: Option<String>,

    /// 风格指导
    pub style_hints: Vec<String>,

    /// 约束条件
    pub constraints: Vec<String>,

    /// 示例（few-shot）
    pub examples: Vec<(String, String)>,
}

impl GenerationGuidance {
    pub fn new() -> Self {
        Self {
            system_guidance: None,
            style_hints: Vec::new(),
            constraints: Vec::new(),
            examples: Vec::new(),
        }
    }

    /// 添加风格提示
    pub fn with_style(mut self, hint: String) -> Self {
        self.style_hints.push(hint);
        self
    }

    /// 添加约束
    pub fn with_constraint(mut self, constraint: String) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// 添加示例
    pub fn with_example(mut self, input: String, output: String) -> Self {
        self.examples.push((input, output));
        self
    }

    /// 转换为系统消息
    pub fn to_system_message(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let mut parts = Vec::new();

        if let Some(guidance) = &self.system_guidance {
            parts.push(guidance.clone());
        }

        if !self.style_hints.is_empty() {
            parts.push(format!("风格要求: {}", self.style_hints.join(", ")));
        }

        if !self.constraints.is_empty() {
            parts.push(format!("约束条件:\n{}", self.constraints.join("\n- ")));
        }

        if !self.examples.is_empty() {
            parts.push("示例:".to_string());
            for (input, output) in &self.examples {
                parts.push(format!("输入: {}\n输出: {}", input, output));
            }
        }

        Some(parts.join("\n\n"))
    }

    fn is_empty(&self) -> bool {
        self.system_guidance.is_none()
            && self.style_hints.is_empty()
            && self.constraints.is_empty()
            && self.examples.is_empty()
    }
}

impl Default for GenerationGuidance {
    fn default() -> Self {
        Self::new()
    }
}

/// 完整的生成控制配置
#[derive(Debug, Clone)]
pub struct GenerationControl {
    /// 生成预设
    pub preset: GenerationPreset,

    /// 自定义参数（覆盖预设）
    pub custom_params: Option<GenerationParams>,

    /// 生成指导
    pub guidance: Option<GenerationGuidance>,

    /// 模型特定参数
    pub model_specific: HashMap<String, Value>,
}

impl GenerationControl {
    pub fn new(preset: GenerationPreset) -> Self {
        Self {
            preset,
            custom_params: None,
            guidance: None,
            model_specific: HashMap::new(),
        }
    }

    /// 设置自定义参数
    pub fn with_params(mut self, params: GenerationParams) -> Self {
        self.custom_params = Some(params);
        self
    }

    /// 设置指导
    pub fn with_guidance(mut self, guidance: GenerationGuidance) -> Self {
        self.guidance = Some(guidance);
        self
    }

    /// 添加模型特定参数
    pub fn with_model_param(mut self, key: String, value: Value) -> Self {
        self.model_specific.insert(key, value);
        self
    }

    /// 获取最终生成参数
    pub fn get_params(&self) -> GenerationParams {
        let base_params = self.preset.to_params();

        if let Some(custom) = &self.custom_params {
            custom.merge(&base_params)
        } else {
            base_params
        }
    }

    /// 应用到 JSON 请求体
    pub fn apply_to_json(&self, json: &mut Value) {
        // 应用生成参数
        let params = self.get_params();
        params.apply_to_json(json);

        // 应用模型特定参数
        for (key, value) in &self.model_specific {
            json[key] = value.clone();
        }
    }

    /// 获取系统消息（如果有指导）
    pub fn get_system_message(&self) -> Option<String> {
        self.guidance.as_ref().and_then(|g| g.to_system_message())
    }
}

impl Default for GenerationControl {
    fn default() -> Self {
        Self::new(GenerationPreset::Balanced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_to_params() {
        let precise = GenerationPreset::Precise.to_params();
        assert_eq!(precise.temperature, Some(0.3));

        let creative = GenerationPreset::Creative.to_params();
        assert_eq!(creative.temperature, Some(0.9));
    }

    #[test]
    fn test_params_validation() {
        let valid = GenerationParams {
            temperature: Some(0.7),
            ..Default::default()
        };
        assert!(valid.validate().is_ok());

        let invalid = GenerationParams {
            temperature: Some(3.0),
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_params_merge() {
        let base = GenerationParams {
            temperature: Some(0.7),
            top_p: Some(0.9),
            frequency_penalty: None,
            presence_penalty: None,
            max_tokens: None,
            stop_sequences: Vec::new(),
            top_k: None,
            repetition_penalty: None,
            length_penalty: None,
        };

        let custom = GenerationParams {
            temperature: Some(0.5),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            max_tokens: None,
            stop_sequences: Vec::new(),
            top_k: None,
            repetition_penalty: None,
            length_penalty: None,
        };

        let merged = custom.merge(&base);
        assert_eq!(merged.temperature, Some(0.5));
        assert_eq!(merged.top_p, Some(0.9));
    }

    #[test]
    fn test_guidance_to_system_message() {
        let guidance = GenerationGuidance::new()
            .with_style("简洁".to_string())
            .with_constraint("不超过100字".to_string())
            .with_example("你好".to_string(), "您好！".to_string());

        let msg = guidance.to_system_message().unwrap();
        assert!(msg.contains("风格要求"));
        assert!(msg.contains("约束条件"));
        assert!(msg.contains("示例"));
    }

    #[test]
    fn test_generation_control() {
        let control =
            GenerationControl::new(GenerationPreset::Precise).with_params(GenerationParams {
                temperature: Some(0.5),
                ..Default::default()
            });

        let params = control.get_params();
        assert_eq!(params.temperature, Some(0.5));
    }

    #[test]
    fn test_apply_to_json() {
        let params = GenerationParams {
            temperature: Some(0.8),
            max_tokens: Some(100),
            ..Default::default()
        };

        let mut json = serde_json::json!({});
        params.apply_to_json(&mut json);

        assert_eq!(json["temperature"], 0.8);
        assert_eq!(json["max_tokens"], 100);
    }
}
