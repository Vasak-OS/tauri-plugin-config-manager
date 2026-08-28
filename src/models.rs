use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VSKConfig {
    /// Con `default` a propósito: a un archivo al que le falte esta sección se
    /// le completa con los valores de fábrica en lugar de darlo por ilegible.
    /// Sin esto, olvidar una clave costaba el archivo entero —con el fondo de
    /// pantalla, los widgets y las fuentes que la persona eligió—, porque el
    /// que no parsea se aparta y se repone.
    #[serde(default)]
    pub style: Style,
    pub desktop: Option<Desktop>,
    #[serde(default)]
    pub fonts: Fonts,
    #[serde(default)]
    pub icons: Icons,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Fonts {
    pub terminal: String,
    pub title: String,
    pub apps: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Icons {
    pub dark: String,
    #[serde(default, alias = "light")]
    pub light: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Desktop {
    #[serde(default)]
    pub wallpaper: Vec<String>,
    #[serde(default = "tamano_de_icono_por_defecto")]
    pub iconsize: u32,
    #[serde(default)]
    pub showfiles: bool,
    #[serde(default)]
    pub showhiddenfiles: bool,
}

fn tamano_de_icono_por_defecto() -> u32 {
    48
}

/// Los valores de fábrica, que son los mismos que escribe la configuración por
/// defecto cuando no hay archivo. Están acá y no duplicados en el escritorio
/// para que no puedan discrepar.
#[derive(Debug, Serialize, Deserialize)]
pub struct Style {
    #[serde(default)]
    pub darkmode: bool,
    #[serde(rename = "color-scheme", default = "esquema_por_defecto")]
    pub color_scheme: String,
    #[serde(default = "radio_por_defecto")]
    pub radius: u32,
}

fn esquema_por_defecto() -> String {
    "vasak-default".to_string()
}

/// En píxeles. El mismo que usa `rounded-corner` en todo el escritorio.
fn radio_por_defecto() -> u32 {
    8
}

impl Default for Style {
    fn default() -> Self {
        Self {
            darkmode: false,
            color_scheme: esquema_por_defecto(),
            radius: radio_por_defecto(),
        }
    }
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
    pub light: ThemeVariant,
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
    pub secondary: String,
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
