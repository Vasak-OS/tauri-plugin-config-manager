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

/// El tamaño del texto más chico que se acepta: por debajo no se lee.
pub const MIN_FONT_SCALE: f64 = 0.75;
/// El más grande: por encima las ventanas de tamaño fijo se rompen.
pub const MAX_FONT_SCALE: f64 = 2.0;

fn default_font_scale() -> f64 {
    1.0
}

/// La escala tal como está escrita, o `1.0` si no es un número.
///
/// Tolerante a propósito: un archivo con `"scale": "grande"` que no parseara
/// costaría el archivo entero —se aparta a `.roto` y se repone, con el fondo y
/// los widgets de quien lo tuviera—, y una escala mal escrita no vale eso.
fn lenient_font_scale<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_f64().unwrap_or_else(default_font_scale))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fonts {
    #[serde(default)]
    pub terminal: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub apps: String,
    /// El tamaño del texto de todo el escritorio, como factor: `1.0` es el de
    /// siempre. Con su propio valor de fábrica —`#[serde(default)]` daría `0.0`,
    /// que dejaría el texto sin tamaño— y sin pasar de [`MIN_FONT_SCALE`] y
    /// [`MAX_FONT_SCALE`] al aplicarse (ver [`Fonts::effective_scale`]).
    #[serde(
        default = "default_font_scale",
        deserialize_with = "lenient_font_scale"
    )]
    pub scale: f64,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            terminal: String::new(),
            title: String::new(),
            apps: String::new(),
            scale: default_font_scale(),
            extra: Default::default(),
        }
    }
}

impl Fonts {
    /// La escala que se aplica: la escrita, entre el mínimo y el máximo, o `1.0`
    /// si no es un número finito.
    pub fn effective_scale(&self) -> f64 {
        if self.scale.is_finite() {
            self.scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE)
        } else {
            default_font_scale()
        }
    }
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
    #[serde(default = "default_icon_size")]
    pub iconsize: u32,
    /// Con su propio valor de fábrica: `#[serde(default)]` daría `false` para un
    /// `bool`, así que a un archivo al que le faltara esta clave se le
    /// esconderían los archivos del escritorio sin que nadie lo pidiera.
    #[serde(default = "default_show_files")]
    pub showfiles: bool,
    #[serde(default)]
    pub showhiddenfiles: bool,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn default_icon_size() -> u32 {
    48
}

fn default_show_files() -> bool {
    true
}

/// Los valores de fábrica, que son los mismos que escribe la configuración por
/// defecto cuando no hay archivo. Están acá y no duplicados en el escritorio
/// para que no puedan discrepar.
#[derive(Debug, Serialize, Deserialize)]
pub struct Style {
    #[serde(default)]
    pub darkmode: bool,
    #[serde(rename = "color-scheme", default = "default_color_scheme")]
    pub color_scheme: String,
    #[serde(default = "default_radius")]
    pub radius: u32,
    /// El borde de afuera de las ventanas, el panel y los emergentes del
    /// escritorio. A un archivo anterior a esta clave se le completa con el
    /// borde de siempre: fino y del color del esquema.
    #[serde(default)]
    pub border: Border,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn default_color_scheme() -> String {
    "vasak-default".to_string()
}

/// En píxeles. El mismo que usa `rounded-corner` en todo el escritorio.
fn default_radius() -> u32 {
    8
}

impl Default for Style {
    fn default() -> Self {
        Self {
            darkmode: false,
            color_scheme: default_color_scheme(),
            radius: default_radius(),
            border: Border::default(),
            extra: serde_json::Map::new(),
        }
    }
}

/// El borde de afuera: el de la ventana entera, el panel, el centro de control
/// y los emergentes del escritorio. Los bordes de adentro de cada aplicación no
/// lo siguen.
///
/// Son cadenas y no enumeraciones a propósito: un valor que el modelo no
/// conozca —una errata a mano, o uno que sume una versión futura— haría
/// ilegible el archivo entero, y el que no parsea se aparta y se repone con
/// los valores de fábrica. Quien lo aplica (`windowBorderProperties`, en
/// `guest-js`) trata lo desconocido como el valor de fábrica.
#[derive(Debug, Serialize, Deserialize)]
pub struct Border {
    /// `normal` (1 px), `thick` (2 px) o `heavy` (3 px).
    #[serde(default = "default_border_width")]
    pub width: String,
    /// `scheme` (el canto translúcido del esquema) o `accent` (el primario).
    #[serde(default = "default_border_color")]
    pub color: String,
    /// Lo que el modelo no conoce, para que sobreviva a la reserialización.
    /// Ver [`VSKConfig::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn default_border_width() -> String {
    "normal".to_string()
}

fn default_border_color() -> String {
    "scheme".to_string()
}

impl Default for Border {
    fn default() -> Self {
        Self {
            width: default_border_width(),
            color: default_border_color(),
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Scheme {
    pub path: String,
    pub scheme: SchemeData,
}

/// Un esquema de color tal como está en su archivo JSON.
///
/// Desde que los esquemas se pueden **guardar** (`save_user_scheme`), este
/// modelo no es sólo de lectura: lo que entra por acá es lo que vuelve al disco.
/// Por eso cada struct del esquema lleva su `extra`, y `text.on-secondary` está
/// declarado.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchemeData {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    pub colors: SchemeColors,
    /// Todo lo que el archivo tenga y el modelo no conozca, tal como está.
    ///
    /// Sin esto, leer un esquema y volver a guardarlo borraba en silencio
    /// cualquier campo que el modelo no declarara: le pasaba ya a
    /// `text.on-secondary`, que `vasak-default.json` trae desde antes de que
    /// existiera acá, y le pasaría a cualquier campo que se sume mañana. Es el
    /// mismo criterio que [`VSKConfig::extra`] para `vasak.conf`.
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SchemeColors {
    pub dark: ThemeVariant,
    pub light: ThemeVariant,
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeVariant {
    pub ui: UiColors,
    pub terminal: TerminalColors,
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UiColors {
    pub color: ColorPalette,
    pub text: TextColors,
    pub background: String,
    pub border: String,
    pub surface: String,
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColorPalette {
    pub primary: String,
    pub secondary: String,
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TextColors {
    pub main: String,
    pub muted: String,
    #[serde(rename = "on-primary")]
    pub on_primary: String,
    /// El texto sobre el color secundario. Opcional porque los esquemas
    /// anteriores no lo traen; cuando falta, no se escribe.
    #[serde(
        rename = "on-secondary",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub on_secondary: Option<String>,
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalColors {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub ansi: AnsiColors,
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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
    /// Lo que el modelo no conoce, para que sobreviva a la ida y vuelta.
    /// Ver [`SchemeData::extra`].
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts(json: &str) -> Fonts {
        serde_json::from_str(json).expect("parsea")
    }

    #[test]
    fn sin_escala_vale_uno() {
        assert_eq!(fonts("{}").scale, 1.0);
        assert_eq!(fonts(r#"{"apps":"Noto Sans"}"#).effective_scale(), 1.0);
        assert_eq!(Fonts::default().scale, 1.0);
    }

    #[test]
    fn la_escala_escrita_se_lee_y_se_conserva_al_reescribir() {
        let leida = fonts(r#"{"scale":1.25}"#);
        assert_eq!(leida.scale, 1.25);

        let reescrita: serde_json::Value = serde_json::to_value(&leida).unwrap();
        assert_eq!(reescrita["scale"], 1.25);
    }

    #[test]
    fn una_escala_que_no_es_un_numero_no_cuesta_el_archivo() {
        // Un archivo que no parsea se aparta y se repone: se llevaría el fondo y
        // los widgets de quien lo tuviera, por una escala mal escrita.
        for raro in [r#""grande""#, "null", "true", "[]", "{}"] {
            let config = fonts(&format!(r#"{{"scale":{raro}}}"#));
            assert_eq!(config.effective_scale(), 1.0, "con {raro}");
        }
    }

    #[test]
    fn la_escala_se_acota_al_aplicarse() {
        assert_eq!(fonts(r#"{"scale":0.1}"#).effective_scale(), MIN_FONT_SCALE);
        assert_eq!(fonts(r#"{"scale":9}"#).effective_scale(), MAX_FONT_SCALE);
        assert_eq!(fonts(r#"{"scale":-3}"#).effective_scale(), MIN_FONT_SCALE);
        let infinita = Fonts {
            scale: f64::NAN,
            ..Fonts::default()
        };
        assert_eq!(infinita.effective_scale(), 1.0);
    }

    #[test]
    fn lo_que_el_modelo_no_conoce_sigue_pasando() {
        let leida = fonts(r#"{"scale":1.5,"otra":"cosa"}"#);

        assert_eq!(leida.extra["otra"], "cosa");
        assert!(!leida.extra.contains_key("scale"));
    }
}
