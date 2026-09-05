//! 双输出模式：human 模式逐行即时打印；--json 模式全程静默、由 emit_value
//! 输出唯一完整负载（单行 compact，与流式事件 emit_event 同形态）。
//! 数据走 stdout、错误走 stderr；human 表格用 pad_display 处理 CJK 宽度对齐。

use serde_json::Value;

pub struct Out {
    pub json: bool,
}

impl Out {
    /// json 模式的唯一出口：单行 compact（与 emit_event 一致，按行消费稳定）；
    /// human 模式不打印（数据已由 kv/line 呈现）
    pub fn emit_value(&self, v: &Value) {
        if self.json {
            println!("{}", serde_json::to_string(v).unwrap_or_default());
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

/// 按显示宽度左对齐（CJK 全角占 2 列），供 human 表格对齐用
pub fn pad_display(s: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let w = s.width();
    if w >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_display_accounts_for_cjk_width() {
        assert_eq!(pad_display("ab", 4), "ab  ");
        assert_eq!(pad_display("中文", 4), "中文");
        assert_eq!(pad_display("中a", 4), "中a ");
        assert_eq!(pad_display("超出宽度", 2), "超出宽度");
    }
}
