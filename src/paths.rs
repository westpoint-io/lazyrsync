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

pub fn complete_path(buffer: &str) -> String {
    let hits = path_hits(buffer);
    match hits.len() {
        0 => buffer.to_string(),
        1 => hits[0].clone(),
        _ => {
            let first = &hits[0];
            let mut len = first.len();
            for h in &hits[1..] {
                len = first
                    .chars()
                    .zip(h.chars())
                    .take_while(|(a, b)| a == b)
                    .count()
                    .min(len);
            }
            first.chars().take(len).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::complete_path;

    #[test]
    fn smart_case_lowercase_query_is_insensitive() {
        let base = std::env::temp_dir().join(format!("lr-comp-i-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("Archive")).unwrap();
        let got = complete_path(&format!("{}/arc", base.display()));
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(got, format!("{}/Archive/", base.display()));
    }

    #[test]
    fn smart_case_uppercase_query_is_sensitive() {
        let base = std::env::temp_dir().join(format!("lr-comp-s-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("archive")).unwrap();
        let typed = format!("{}/Arc", base.display());
        let got = complete_path(&typed);
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(got, typed);
    }

    #[test]
    fn remote_paths_are_left_untouched() {
        assert_eq!(complete_path("me@vps:/backup/da"), "me@vps:/backup/da");
    }
}
