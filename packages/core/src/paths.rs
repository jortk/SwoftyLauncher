use std::path::PathBuf;

pub const ENV: &str = "SWOFTY_DATA_DIR";

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(ENV) {
        return PathBuf::from(dir);
    }
    platform_data_dir()
}

#[cfg(windows)]
pub fn platform_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Swofty")
}

#[cfg(not(windows))]
pub fn platform_data_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let flatpak = home.join(".var/app/com.valvesoftware.Steam/data");
    if flatpak.is_dir() && !home.join(".local/share/Steam").exists() {
        return flatpak.join("swofty");
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("swofty")
}

pub fn dangling_link(path: &std::path::Path) -> Option<PathBuf> {
    let link = path.symlink_metadata().ok()?.file_type().is_symlink();
    if !link || path.metadata().is_ok() {
        return None;
    }
    std::fs::read_link(path).ok()
}

pub fn game_folder_view(path: &std::path::Path) -> String {
    if cfg!(windows) {
        return path.display().to_string();
    }
    format!("Z:{}", path.display().to_string().replace('/', "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn finds_a_link_whose_target_is_gone() {
        let dir = std::env::temp_dir().join(format!("swofty-paths-dangling-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let (target, at) = (dir.join("target"), dir.join("link"));
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, &at).unwrap();
        assert_eq!(dangling_link(&at), None);
        assert_eq!(dangling_link(&target), None);
        std::fs::remove_dir(&target).unwrap();
        assert_eq!(dangling_link(&at), Some(target));
        assert_eq!(dangling_link(&dir.join("absent")), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn proton_view_uses_z_drive() {
        if cfg!(windows) {
            return;
        }
        assert_eq!(
            game_folder_view(std::path::Path::new("/home/a/b")),
            "Z:\\home\\a\\b"
        );
    }
}
