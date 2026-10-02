const REPOSITORY: &str = "https://github.com/ABelli99/Steroitor";

/// Apre nel browser una pagina del repository di Steroitor (e nient'altro).
#[tauri::command]
pub fn open_release_page(url: String) -> Result<(), String> {
    if !url.starts_with(&format!("{REPOSITORY}/")) {
        return Err("Indirizzo non consentito".into());
    }
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program).arg(&url).spawn().map(drop).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_to_open_pages_outside_the_repository() {
        assert!(open_release_page("https://example.com/malware.exe".into()).is_err());
        assert!(open_release_page(format!("{REPOSITORY}.evil.com/x")).is_err());
    }
}
