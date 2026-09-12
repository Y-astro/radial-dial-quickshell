// src/state/context.rs

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Context {
    Default,
    Terminal, // was "kitty" in QML
    Browser,
    Code,
    Media,
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Context::Default => write!(f, "default"),
            Context::Terminal => write!(f, "kitty"),
            Context::Browser => write!(f, "browser"),
            Context::Code => write!(f, "code"),
            Context::Media => write!(f, "media"),
        }
    }
}

/// Port of resolveContext(win) from RadialMenuActions.qml lines 123-132
pub fn resolve_context(window_class: &str, title: &str) -> Context {
    let c = window_class.to_lowercase();
    let t = title.to_lowercase();

    // Terminal: kitty, konsole, alacritty, foot (order matches QML)
    if c.contains("kitty") || c.contains("konsole") || c.contains("alacritty") || c.contains("foot") {
        return Context::Terminal;
    }

    // Browser regex: /firefox|brave|chrome|chromium|thorium|zen|floorp|vivaldi|opera|edge|librewolf|waterfox/
    let browser_patterns = [
        "firefox", "brave", "chrome", "chromium", "thorium",
        "zen", "floorp", "vivaldi", "opera", "edge",
        "librewolf", "waterfox",
    ];
    if browser_patterns.iter().any(|p| c.contains(p) || t.contains(p)) {
        return Context::Browser;
    }

    // Code: /code|cursor|antigravity|vscodium|nvim|neovim/
    if ["code", "cursor", "antigravity", "vscodium", "nvim", "neovim"].iter().any(|p| c.contains(p)) {
        return Context::Code;
    }

    // Media: /mpv|spotify|vlc/
    if ["mpv", "spotify", "vlc"].iter().any(|p| c.contains(p)) {
        return Context::Media;
    }

    Context::Default
}

/// Check if window is in special workspace (port of isInSpecialWorkspace)
pub fn is_in_special_workspace(workspace_id: i64, workspace_name: &str) -> bool {
    workspace_name.starts_with("special") || workspace_id < 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_context_terminal() {
        assert_eq!(resolve_context("kitty", ""), Context::Terminal);
        assert_eq!(resolve_context("Alacritty", ""), Context::Terminal);
        assert_eq!(resolve_context("foot", ""), Context::Terminal);
        assert_eq!(resolve_context("konsole", ""), Context::Terminal);
    }

    #[test]
    fn test_resolve_context_browser() {
        assert_eq!(resolve_context("firefox", ""), Context::Browser);
        assert_eq!(resolve_context("zen-browser", ""), Context::Browser);
        assert_eq!(resolve_context("Google-chrome", ""), Context::Browser);
        assert_eq!(resolve_context("brave-browser", ""), Context::Browser);
        assert_eq!(resolve_context("", "firefox private browsing"), Context::Browser);
    }

    #[test]
    fn test_resolve_context_code() {
        assert_eq!(resolve_context("code", ""), Context::Code);
        assert_eq!(resolve_context("Cursor", ""), Context::Code);
        assert_eq!(resolve_context("nvim", ""), Context::Code);
        assert_eq!(resolve_context("antigravity", ""), Context::Code);
        assert_eq!(resolve_context("vscodium", ""), Context::Code);
    }

    #[test]
    fn test_resolve_context_media() {
        assert_eq!(resolve_context("spotify", ""), Context::Media);
        assert_eq!(resolve_context("mpv", ""), Context::Media);
        assert_eq!(resolve_context("vlc", ""), Context::Media);
    }

    #[test]
    fn test_resolve_context_default() {
        assert_eq!(resolve_context("steam", ""), Context::Default);
        assert_eq!(resolve_context("", ""), Context::Default);
    }

    #[test]
    fn test_special_workspace() {
        assert!(is_in_special_workspace(-1, "special:scratchpad"));
        assert!(is_in_special_workspace(1, "special:magic"));
        assert!(!is_in_special_workspace(1, "1"));
    }

    #[test]
    fn test_context_display() {
        assert_eq!(Context::Default.to_string(), "default");
        assert_eq!(Context::Terminal.to_string(), "kitty");
        assert_eq!(Context::Browser.to_string(), "browser");
        assert_eq!(Context::Code.to_string(), "code");
        assert_eq!(Context::Media.to_string(), "media");
    }
}
