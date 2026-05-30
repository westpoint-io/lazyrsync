pub fn expand_tilde(s: &str) -> String {
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());
    match (s, home) {
        ("~", Some(home)) => home,
        (p, Some(home)) if p.starts_with("~/") => format!("{home}/{}", &p[2..]),
        _ => s.to_string(),
    }
}

pub fn path_hits(buffer: &str) -> Vec<String> {
    if buffer.contains('@') || (buffer.contains(':') && !buffer.starts_with('/')) {
        return Vec::new();
    }
    let (dir_disp, frag) = match buffer.rsplit_once('/') {
        Some((d, f)) => (d.to_string(), f.to_string()),
        None => (String::new(), buffer.to_string()),
    };
    let dir_real = if dir_disp.is_empty() {
        if buffer.starts_with('/') {
            "/".to_string()
        } else {
            ".".to_string()
        }
    } else {
        expand_tilde(&dir_disp)
    };
    let smart_sensitive = frag.chars().any(|c| c.is_uppercase());
    let frag_lower = frag.to_lowercase();
    let mut hits: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir_real) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !frag.starts_with('.') && name.starts_with('.') {
                continue;
            }
            let matches = if smart_sensitive {
                name.starts_with(&frag)
            } else {
                name.to_lowercase().starts_with(&frag_lower)
            };
            if matches {
                let mut full = if dir_disp.is_empty() {
                    if buffer.starts_with('/') {
                        format!("/{name}")
                    } else {
                        name.clone()
                    }
                } else {
                    format!("{dir_disp}/{name}")
                };
                if entry.path().is_dir() {
                    full.push('/');
                }
                hits.push(full);
            }
        }
    }
    hits.sort();
    hits
}
