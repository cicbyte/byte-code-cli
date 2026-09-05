//! 双输出模式：human 模式逐行即时打印；--json 模式全程静默、由 emit_value
//! 输出唯一完整负载（payload 组装在调用方，kv/line 不再自带重复 json）。

use serde_json::Value;

pub struct Out {
    pub json: bool,
}

impl Out {
    /// json 模式的唯一出口；human 模式不打印（数据已由 kv/line 呈现）
    pub fn emit_value(&self, v: &Value) {
        if self.json {
            println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
        }
    }

    /// 流式事件输出（notify --watch / comments --follow）：json 模式一行一事件
    pub fn emit_event(&self, v: &Value) {
        if self.json {
            println!("{}", serde_json::to_string(v).unwrap_or_default());
        }
    }

    /// 仅 human 模式打印的说明行
    pub fn line(&self, human: &str) {
        if !self.json {
            println!("{human}");
        }
    }

    pub fn kv(&self, key: &str, value: &str) {
        self.line(&format!("  {key:<14} {value}"));
    }
}
