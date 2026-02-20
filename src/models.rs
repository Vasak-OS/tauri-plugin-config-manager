use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VSKConfig {
    pub style: Style,
    pub desktop: Option<Desktop>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Desktop {
    pub wallpaper: Vec<String>,
    pub iconsize: u32,
    pub showfiles: bool,
    pub showhiddenfiles: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Style {
    pub darkmode: bool,
    #[serde(rename = "color-scheme")]
    pub color_scheme: String,
    pub radius: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Scheme {
    pub path: String,
    pub scheme: SchemeData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchemeData {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    pub colors: SchemeColors,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchemeColors {
    pub dark: ThemeVariant,
    pub ligth: ThemeVariant,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeVariant {
    pub ui: UiColors,
    pub terminal: TerminalColors,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiColors {
    pub color: ColorPalette,
    pub text: TextColors,
    pub background: String,
    pub border: String,
    pub surface: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColorPalette {
    pub primary: String,
    pub seccondary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TextColors {
    pub main: String,
    pub muted: String,
    #[serde(rename = "on-primary")]
    pub on_primary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalColors {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub ansi: AnsiColors,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnsiColors {
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    #[serde(rename = "brightBlack")]
    pub bright_black: String,
    #[serde(rename = "brightRed")]
    pub bright_red: String,
    #[serde(rename = "brightGreen")]
    pub bright_green: String,
    #[serde(rename = "brightYellow")]
    pub bright_yellow: String,
    #[serde(rename = "brightBlue")]
    pub bright_blue: String,
    #[serde(rename = "brightMagenta")]
    pub bright_magenta: String,
    #[serde(rename = "brightCyan")]
    pub bright_cyan: String,
    #[serde(rename = "brightWhite")]
    pub bright_white: String,
}
