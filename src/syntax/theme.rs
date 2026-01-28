use egui::Color32;

#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    pub keyword: Color32,
    pub function: Color32,
    pub function_call: Color32,
    pub type_name: Color32,
    pub string: Color32,
    pub number: Color32,
    pub comment: Color32,
    pub operator: Color32,
    pub punctuation: Color32,
    pub variable: Color32,
    pub constant: Color32,
    pub default: Color32,
}

impl SyntaxTheme {
    /// Dark theme (inspired by One Dark)
    pub fn dark() -> Self {
        Self {
            keyword: Color32::from_rgb(198, 120, 221),      // Purple
            function: Color32::from_rgb(97, 175, 239),      // Blue
            function_call: Color32::from_rgb(97, 175, 239), // Blue
            type_name: Color32::from_rgb(229, 192, 123),    // Yellow
            string: Color32::from_rgb(152, 195, 121),       // Green
            number: Color32::from_rgb(209, 154, 102),       // Orange
            comment: Color32::from_rgb(92, 99, 112),        // Gray
            operator: Color32::from_rgb(86, 182, 194),      // Cyan
            punctuation: Color32::from_rgb(171, 178, 191),  // Light gray
            variable: Color32::from_rgb(224, 108, 117),     // Red
            constant: Color32::from_rgb(209, 154, 102),     // Orange
            default: Color32::from_rgb(171, 178, 191),      // Light gray
        }
    }

    /// Light theme (inspired by One Light)
    pub fn light() -> Self {
        Self {
            keyword: Color32::from_rgb(166, 38, 164),       // Purple
            function: Color32::from_rgb(64, 120, 242),      // Blue
            function_call: Color32::from_rgb(64, 120, 242), // Blue
            type_name: Color32::from_rgb(193, 132, 1),      // Yellow
            string: Color32::from_rgb(80, 161, 79),         // Green
            number: Color32::from_rgb(152, 104, 1),         // Orange
            comment: Color32::from_rgb(160, 161, 167),      // Gray
            operator: Color32::from_rgb(0, 132, 137),       // Cyan
            punctuation: Color32::from_rgb(56, 58, 66),     // Dark gray
            variable: Color32::from_rgb(228, 86, 73),       // Red
            constant: Color32::from_rgb(152, 104, 1),       // Orange
            default: Color32::from_rgb(56, 58, 66),         // Dark gray
        }
    }

    pub fn get_color(&self, capture_name: &str) -> Color32 {
        match capture_name {
            "keyword" => self.keyword,
            "function" | "function.method" => self.function,
            "function.call" | "function.macro" => self.function_call,
            "type" | "type.builtin" => self.type_name,
            "string" => self.string,
            "number" => self.number,
            "comment" => self.comment,
            "operator" => self.operator,
            "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => self.punctuation,
            "variable" => self.variable,
            "constant" | "constant.builtin" => self.constant,
            _ => self.default,
        }
    }
}
