//! Mirrors the appearance into the files GTK reads when it starts.
//!
//! Dark mode and the icon pack were only ever handed to `gsettings`, under the
//! `org.gnome.desktop.interface` schema. On GNOME a settings daemon watches that
//! schema and pushes the values into every running GTK application; on a VasakOS
//! session there is no such daemon, so nothing reads them back. The applications
//! that happened to look right did so only because they were told directly while
//! they were running — after a reboot every GTK program came up light-themed
//! with the default icons, next to a desktop that was still dark.
//!
//! GTK's own persistent store is `settings.ini`, one per major version, and that
//! is what this writes. `gsettings` is still written as well: it is what the
//! portal reports to sandboxed applications.
//!
//! The files belong to the user, not to us: only the keys below are touched, and
//! every other key, section and comment in them is preserved.

use std::path::PathBuf;

/// The GTK versions VasakOS applications are built against.
const GTK_CONFIG_DIRS: &[&str] = &["gtk-3.0", "gtk-4.0"];

/// The theme applied for each mode.
///
/// Kept beside the `gsettings` sync, which sets the same names: the two stores
/// disagreeing is the whole failure being fixed here.
pub const DARK_GTK_THEME: &str = "Adwaita-dark";
pub const LIGHT_GTK_THEME: &str = "Adwaita";

fn config_home() -> Option<PathBuf> {
    // GTK honours XDG_CONFIG_HOME when it looks for settings.ini, so writing to
    // a hardcoded ~/.config would land beside the file it actually reads.
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(dir);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }

    home::home_dir().map(|home| home.join(".config"))
}

/// Rewrites `[Settings]` with `updates`, leaving the rest of the file as it was.
///
/// Keys already present are replaced in place, missing ones are appended to the
/// section, and a file without a `[Settings]` section gets one. Anything else —
/// other sections, unknown keys, comments, blank lines — comes through
/// untouched, because this file is one a person may well have written by hand.
fn merged_settings(existing: &str, updates: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut replaced: Vec<&str> = Vec::new();
    let mut in_settings = false;
    let mut settings_ends_at: Option<usize> = None;

    for line in existing.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            if in_settings && settings_ends_at.is_none() {
                settings_ends_at = Some(lines.len());
            }
            in_settings = trimmed.eq_ignore_ascii_case("[settings]");
            lines.push(line.to_string());
            continue;
        }

        if in_settings {
            let key = trimmed.split('=').next().unwrap_or("").trim();
            if let Some((name, value)) = updates
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
            {
                lines.push(format!("{name}={value}"));
                replaced.push(name);
                continue;
            }
        }

        lines.push(line.to_string());
    }

    if in_settings && settings_ends_at.is_none() {
        settings_ends_at = Some(lines.len());
    }

    let missing: Vec<String> = updates
        .iter()
        .filter(|(name, _)| !replaced.iter().any(|written| written == name))
        .map(|(name, value)| format!("{name}={value}"))
        .collect();

    match settings_ends_at {
        Some(at) => {
            for (offset, line) in missing.into_iter().enumerate() {
                lines.insert(at + offset, line);
            }
        }
        None => {
            if lines.last().is_some_and(|last| !last.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push("[Settings]".to_string());
            lines.extend(missing);
        }
    }

    let mut result = lines.join("\n");
    result.push('\n');
    result
}

/// Replaces the file in one step, so a program reading it mid-write never sees
/// half a configuration and falls back to its defaults.
fn write_atomically(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    let temporary = path.with_extension("ini.vasak-tmp");

    std::fs::write(&temporary, content)?;
    if let Err(e) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(e);
    }

    Ok(())
}

/// Records the appearance for the next time a GTK application starts.
///
/// `icon_pack` empty means the configuration names no pack for this mode; the
/// key is then left alone rather than blanked, since an empty icon theme leaves
/// applications with no icons at all.
pub fn apply(darkmode: bool, icon_pack: &str) {
    let Some(config_home) = config_home() else {
        tracing::warn!("No home directory; skipping GTK settings.ini update");
        return;
    };

    let mut updates: Vec<(&str, &str)> = vec![
        (
            "gtk-theme-name",
            if darkmode {
                DARK_GTK_THEME
            } else {
                LIGHT_GTK_THEME
            },
        ),
        // `true`/`false` rather than `1`/`0`: both are valid, and this is the
        // spelling every other tool that edits this file uses.
        (
            "gtk-application-prefer-dark-theme",
            if darkmode { "true" } else { "false" },
        ),
    ];

    let icon_pack = icon_pack.trim();
    if !icon_pack.is_empty() {
        updates.push(("gtk-icon-theme-name", icon_pack));
    }

    for directory in GTK_CONFIG_DIRS {
        let directory = config_home.join(directory);
        if let Err(e) = std::fs::create_dir_all(&directory) {
            tracing::warn!(
                "Could not create GTK config directory {}: {}",
                directory.display(),
                e
            );
            continue;
        }

        let path = directory.join("settings.ini");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let merged = merged_settings(&existing, &updates);

        // Nothing to do is the common case: the toggle is flipped far more often
        // than the file needs to change, and rewriting it would wake up every
        // GTK application watching it for nothing.
        if merged == existing {
            continue;
        }

        if let Err(e) = write_atomically(&path, &merged) {
            tracing::warn!("Could not write {}: {}", path.display(), e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATES: &[(&str, &str)] = &[
        ("gtk-theme-name", "Adwaita-dark"),
        ("gtk-application-prefer-dark-theme", "true"),
        ("gtk-icon-theme-name", "VasakOS"),
    ];

    #[test]
    fn an_empty_file_gets_a_settings_section() {
        let result = merged_settings("", UPDATES);

        assert_eq!(
            result,
            "[Settings]\ngtk-theme-name=Adwaita-dark\n\
             gtk-application-prefer-dark-theme=true\ngtk-icon-theme-name=VasakOS\n"
        );
    }

    #[test]
    fn existing_values_are_replaced_in_place() {
        let existing = "[Settings]\ngtk-theme-name=Adwaita\ngtk-font-name=Cantarell 11\n";

        let result = merged_settings(existing, UPDATES);

        assert!(result.contains("gtk-theme-name=Adwaita-dark"));
        assert!(!result.contains("gtk-theme-name=Adwaita\n"));
        // Somebody else's key survives.
        assert!(result.contains("gtk-font-name=Cantarell 11"));
    }

    #[test]
    fn keys_are_matched_however_they_were_written() {
        let existing = "[Settings]\n  GTK-Theme-Name  =  Adwaita  \n";

        let result = merged_settings(existing, UPDATES);

        assert!(result.contains("gtk-theme-name=Adwaita-dark"));
        assert!(!result.contains("Adwaita  "));
    }

    /// The user's own file is not ours to rewrite: comments and other sections
    /// have to come out the way they went in.
    #[test]
    fn comments_and_other_sections_are_preserved() {
        let existing = "# escrito a mano\n[Settings]\ngtk-font-name=Sans 10\n\n\
                        [Debug]\nenable-inspector-keybinding=true\n";

        let result = merged_settings(existing, UPDATES);

        assert!(result.starts_with("# escrito a mano\n"));
        assert!(result.contains("[Debug]\nenable-inspector-keybinding=true"));
        assert!(result.contains("gtk-font-name=Sans 10"));
    }

    /// New keys go inside `[Settings]`, not after the section that follows it —
    /// GTK would ignore them there.
    #[test]
    fn missing_keys_are_added_inside_the_settings_section() {
        let existing = "[Settings]\ngtk-font-name=Sans 10\n[Debug]\nenable-inspector-keybinding=true\n";

        let result = merged_settings(existing, UPDATES);
        let settings_block = result.split("[Debug]").next().unwrap();

        assert!(settings_block.contains("gtk-theme-name=Adwaita-dark"));
        assert!(settings_block.contains("gtk-icon-theme-name=VasakOS"));
    }

    /// A key of ours living outside `[Settings]` belongs to another section and
    /// must not be hijacked.
    #[test]
    fn keys_in_other_sections_are_left_alone() {
        let existing = "[Other]\ngtk-theme-name=SomethingElse\n";

        let result = merged_settings(existing, UPDATES);

        assert!(result.contains("[Other]\ngtk-theme-name=SomethingElse"));
        assert!(result.contains("[Settings]"));
    }

    #[test]
    fn writing_the_same_appearance_twice_changes_nothing() {
        let once = merged_settings("", UPDATES);
        let twice = merged_settings(&once, UPDATES);

        assert_eq!(once, twice);
    }

    /// The only test that touches the environment, so nothing else can race it.
    #[test]
    fn both_gtk_versions_get_the_appearance_on_disk() {
        let root = std::env::temp_dir().join(format!("vasak-gtk-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::env::set_var("XDG_CONFIG_HOME", &root);

        // A real file, as other tools leave it: cursor, fonts, modules and the
        // rest have to still be there afterwards.
        std::fs::create_dir_all(root.join("gtk-3.0")).unwrap();
        std::fs::write(
            root.join("gtk-3.0/settings.ini"),
            "[Settings]\n\
             gtk-application-prefer-dark-theme=true\n\
             gtk-cursor-theme-name=WhiteSur-cursors\n\
             gtk-font-name=Noto Sans,  10\n\
             gtk-icon-theme-name=Papirus\n\
             gtk-modules=colorreload-gtk-module\n\
             gtk-theme-name=Breeze\n\
             gtk-xft-dpi=98304\n",
        )
        .unwrap();

        apply(true, "VasakOS");

        let written = std::fs::read_to_string(root.join("gtk-3.0/settings.ini")).unwrap();
        for untouched in [
            "gtk-cursor-theme-name=WhiteSur-cursors",
            "gtk-font-name=Noto Sans,  10",
            "gtk-modules=colorreload-gtk-module",
            "gtk-xft-dpi=98304",
        ] {
            assert!(written.contains(untouched), "perdimos: {untouched}");
        }
        assert!(!written.contains("Breeze"));
        assert!(!written.contains("Papirus"));

        for directory in GTK_CONFIG_DIRS {
            let written = std::fs::read_to_string(root.join(directory).join("settings.ini"))
                .unwrap_or_else(|e| panic!("{directory}/settings.ini: {e}"));

            assert!(written.contains("gtk-theme-name=Adwaita-dark"), "{directory}");
            assert!(written.contains("gtk-icon-theme-name=VasakOS"), "{directory}");
            assert!(
                written.contains("gtk-application-prefer-dark-theme=true"),
                "{directory}"
            );
        }

        // Switching back leaves no trace of the dark theme behind.
        apply(false, "VasakOS-light");
        let written = std::fs::read_to_string(root.join("gtk-3.0/settings.ini")).unwrap();
        assert!(written.contains("gtk-theme-name=Adwaita\n"));
        assert!(written.contains("gtk-application-prefer-dark-theme=false"));
        assert!(written.contains("gtk-icon-theme-name=VasakOS-light"));

        // A mode with no icon pack configured keeps the one already there rather
        // than leaving applications with no icons at all.
        apply(true, "   ");
        let written = std::fs::read_to_string(root.join("gtk-3.0/settings.ini")).unwrap();
        assert!(written.contains("gtk-icon-theme-name=VasakOS-light"));

        // Nothing is left behind by the atomic replace.
        let leftovers: Vec<_> = std::fs::read_dir(root.join("gtk-3.0"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name())
            .filter(|name| name != "settings.ini")
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");

        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn light_mode_is_written_as_explicitly_as_dark_mode() {
        let updates = &[
            ("gtk-theme-name", LIGHT_GTK_THEME),
            ("gtk-application-prefer-dark-theme", "false"),
        ];

        let result = merged_settings("[Settings]\ngtk-application-prefer-dark-theme=true\n", updates);

        assert!(result.contains("gtk-application-prefer-dark-theme=false"));
        assert!(!result.contains("prefer-dark-theme=true"));
    }
}
