use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Settings {
    pub show_hidden: bool,
    pub sort: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_hidden: false,
            sort: "relevance".into(),
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dir = home.join(".config/file-explorer");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("settings.txt"))
}

pub fn save(s: &Settings) {
    if let Some(p) = settings_path() {
        let body = format!("show_hidden={}\nsort={}\n", s.show_hidden, s.sort);
        let _ = std::fs::write(p, body);
    }
}

pub fn load() -> Settings {
    let mut s = Settings::default();
    if let Some(p) = settings_path() {
        if let Ok(body) = std::fs::read_to_string(p) {
            for line in body.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (k, v) = match line.split_once('=') {
                    Some(p) => p,
                    None => continue,
                };
                match k.trim() {
                    "show_hidden" => {
                        s.show_hidden = matches!(v.trim(), "true" | "1" | "yes");
                    }
                    "sort" => s.sort = v.trim().to_string(),
                    _ => {}
                }
            }
        }
    }
    s
}
