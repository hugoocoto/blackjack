//! Command-line flags. `--install-desktop` adds a menu entry and icon for the
//! current user, for installs that only ship the binary (like `cargo install`).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, io};

const DESKTOP: &str = include_str!("../packaging/linux/hushjack.desktop");
const SVG: &[u8] = include_bytes!("../assets/icon/hushjack.svg");
const PNG: &[u8] = include_bytes!("../assets/icon/hushjack.png");

const USAGE: &str = "\
Usage: hushjack [option]

With no option, starts the game.

  --seed N              play a fixed shoe: the same seed deals the same cards
  --install-desktop     add Hushjack to your app menu (Linux)
  --uninstall-desktop   remove it again
  -h, --help            show this help
  -V, --version         show the version";

/// The shuffle seed from `--seed N` (or `--seed=N`), if given.
pub fn seed() -> Option<u64> {
    let args: Vec<String> = env::args().collect();
    args.iter().enumerate().find_map(|(i, a)| match a.strip_prefix("--seed") {
        Some("") => args.get(i + 1)?.parse().ok(),
        Some(rest) => rest.strip_prefix('=')?.parse().ok(),
        None => None,
    })
}

/// Handles a command-line flag and returns the exit code, or None to start the game.
pub fn run_flags() -> Option<i32> {
    let arg = env::args().nth(1)?;
    let result = match arg.as_str() {
        "--install-desktop" => install(),
        "--uninstall-desktop" => uninstall(),
        "-h" | "--help" => {
            println!("{USAGE}");
            return Some(0);
        }
        "-V" | "--version" => {
            println!("hushjack {}", env!("CARGO_PKG_VERSION"));
            return Some(0);
        }
        // Anything else (e.g. what a launcher may pass) just starts the game.
        _ => return None,
    };
    Some(match result {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(e) => {
            eprintln!("hushjack: {e}");
            1
        }
    })
}

/// Where the files go: under $XDG_DATA_HOME, or ~/.local/share.
struct Paths {
    apps: PathBuf,
    icons: PathBuf,
    desktop: PathBuf,
    svg: PathBuf,
    png: PathBuf,
}

fn paths() -> io::Result<Paths> {
    if cfg!(any(windows, target_os = "macos")) {
        return Err(io::Error::other("menu entries are only supported on Linux"));
    }
    let data = match env::var_os("XDG_DATA_HOME").filter(|v| Path::new(v).is_absolute()) {
        Some(d) => PathBuf::from(d),
        None => {
            let home = env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?;
            PathBuf::from(home).join(".local/share")
        }
    };
    let apps = data.join("applications");
    let icons = data.join("icons/hicolor");
    Ok(Paths {
        desktop: apps.join("hushjack.desktop"),
        svg: icons.join("scalable/apps/hushjack.svg"),
        png: icons.join("256x256/apps/hushjack.png"),
        apps,
        icons,
    })
}

fn install() -> io::Result<String> {
    let p = paths()?;
    // The menu may not have the binary's folder (e.g. ~/.cargo/bin) on its
    // PATH, so the entry points at this exact file.
    let exe = env::current_exe()?;
    let exe = exe.to_str().ok_or_else(|| io::Error::other("the game's path is not valid UTF-8"))?;
    let entry = DESKTOP.replace("\nExec=hushjack\n", &format!("\nExec={}\n", exec_quote(exe)));
    for (path, bytes) in [(&p.desktop, entry.as_bytes()), (&p.svg, SVG), (&p.png, PNG)] {
        fs::create_dir_all(path.parent().unwrap())
            .and_then(|_| fs::write(path, bytes))
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
    }
    refresh(&p);
    Ok(format!("Added Hushjack to your app menu ({}).", p.desktop.display()))
}

fn uninstall() -> io::Result<String> {
    let p = paths()?;
    for path in [&p.desktop, &p.svg, &p.png] {
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    refresh(&p);
    Ok("Removed Hushjack from your app menu.".into())
}

/// A path as a quoted Exec argument. The spec unescapes the line as a string
/// first, so the quoting backslash is itself written escaped (`\\$`, and
/// four backslashes for one).
fn exec_quote(path: &str) -> String {
    let mut s = String::from("\"");
    for c in path.chars() {
        match c {
            '"' | '`' | '$' => {
                s.push_str("\\\\");
                s.push(c);
            }
            '\\' => s.push_str("\\\\\\\\"),
            '%' => s.push_str("%%"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Lets menus notice the change sooner. Both tools are optional.
fn refresh(p: &Paths) {
    let _ = Command::new("update-desktop-database").arg("-q").arg(&p.apps).status();
    // Only refresh an existing cache: creating one would hide icons that other
    // apps add later without updating it.
    if p.icons.join("icon-theme.cache").exists() {
        let _ = Command::new("gtk-update-icon-cache").arg("-q").arg(&p.icons).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_quote_escapes() {
        assert_eq!(exec_quote("/home/a b/.cargo/bin/hushjack"), "\"/home/a b/.cargo/bin/hushjack\"");
        assert_eq!(exec_quote(r#"/x/$y"%`\"#), r#""/x/\\$y\\"%%\\`\\\\""#);
    }

    #[test]
    fn desktop_file_has_the_exec_line_install_replaces() {
        assert!(DESKTOP.contains("\nExec=hushjack\n"));
    }
}
