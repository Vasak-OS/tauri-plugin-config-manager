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
    #[serde(default)]
    pub desktop: Option<Desktop>,
    #[serde(default)]
    pub fonts: Fonts,
    #[serde(default)]
    pub icons: Icons,
    /// Todo lo demás que haya en el archivo, tal como está.
    ///
    /// El modelo no conoce todas las claves de la configuración —los widgets del
    /// escritorio, por ejemplo, los escribe el escritorio y los lee sólo él—, y
    /// hasta ahora lo que no estaba acá se perdía: `read_config` devuelve la
    /// configuración **reserializada** desde este modelo, así que cada lectura
    /// borraba esas claves de lo que ve la interfaz, y el próximo `writeConfig`
    /// —cambiar el tema, la fuente, cualquier cosa en Ajustes— las borraba del
    /// disco. Los widgets acomodados volvían a la disposición de fábrica.
    ///
    /// Con esto, lo que el modelo no conoce entra, sale y vuelve al archivo
    /// igual que estaba.
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Fonts {
    #[serde(default)]
    pub terminal: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub apps: String,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Icons {
    #[serde(default)]
    pub dark: String,
    #[serde(default, alias = "light")]
    pub light: String,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Desktop {
    #[serde(default)]
    pub wallpaper: Vec<String>,
    #[serde(default = "tamano_de_icono_por_defecto")]
    pub iconsize: u32,
    /// Con su propio valor de fábrica: `#[serde(default)]` daría `false` para un
    /// `bool`, así que a un archivo al que le faltara esta clave se le
    /// esconderían los archivos del escritorio sin que nadie lo pidiera.
    #[serde(default = "mostrar_archivos_por_defecto")]
    pub showfiles: bool,
    #[serde(default)]
    pub showhiddenfiles: bool,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn tamano_de_icono_por_defecto() -> u32 {
    48
}

fn mostrar_archivos_por_defecto() -> bool {
    true
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
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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
            extra: serde_json::Map::new(),
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
