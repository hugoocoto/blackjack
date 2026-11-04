//! What the game keeps between sessions, in small text files in the user's
//! data folder (in the browser, in its local storage): the chosen looks, any
//! custom ones, and the records board (the best wins and the worst losses of
//! whole sessions).

use crate::theme::{Look, Themes};
use std::path::PathBuf;
use std::{env, fs};

/// How many entries each side of the records board keeps.
pub const BOARD: usize = 5;

/// The per-user folder for the game's files, or None where there is none
/// (the web build, see `web`; Android, where the app's own folder isn't
/// wired up yet).
fn dir() -> Option<PathBuf> {
    if cfg!(any(target_arch = "wasm32", target_os = "android")) {
        return None;
    }
    let var = |k: &str| env::var_os(k).map(PathBuf::from);
    let base = if cfg!(windows) {
        var("APPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library/Application Support")
    } else {
        var("XDG_DATA_HOME").filter(|p| p.is_absolute()).or_else(|| Some(var("HOME")?.join(".local/share")))?
    };
    Some(base.join("hushjack"))
}

fn read(name: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    return web::read(name);
    dir().and_then(|d| fs::read_to_string(d.join(name)).ok()).unwrap_or_default()
}

fn write(name: &str, text: &str) {
    #[cfg(target_arch = "wasm32")]
    return web::write(name, text);
    let Some(d) = dir() else { return };
    if let Err(e) = fs::create_dir_all(&d).and_then(|_| fs::write(d.join(name), text)) {
        eprintln!("hushjack: couldn't save {name}: {e}");
    }
}

/// The browser's local storage, through the functions `web/index.html` adds.
#[cfg(target_arch = "wasm32")]
mod web {
    unsafe extern "C" {
        fn hj_storage_len(key: *const u8, key_len: usize) -> i32;
        fn hj_storage_read(key: *const u8, key_len: usize, buf: *mut u8);
        fn hj_storage_write(key: *const u8, key_len: usize, text: *const u8, text_len: usize);
    }

    pub fn read(name: &str) -> String {
        // SAFETY: the pointers and lengths come from live slices, and the
        // buffer is as long as the length the page reported.
        unsafe {
            let Ok(len) = usize::try_from(hj_storage_len(name.as_ptr(), name.len())) else { return String::new() };
            let mut buf = vec![0u8; len];
            hj_storage_read(name.as_ptr(), name.len(), buf.as_mut_ptr());
            String::from_utf8(buf).unwrap_or_default()
        }
    }

    pub fn write(name: &str, text: &str) {
        // SAFETY: the pointers and lengths come from live slices.
        unsafe { hj_storage_write(name.as_ptr(), name.len(), text.as_ptr(), text.len()) }
    }
}

/// The built-in looks plus the ones in `themes.txt` (see
/// `Themes::add_custom`); lines it can't read are reported and skipped.
pub fn load_themes() -> Themes {
    let mut themes = Themes::default();
    for e in themes.add_custom(&read("themes.txt")) {
        eprintln!("hushjack: {e}");
    }
    themes
}

/// The chosen looks (see `theme`).
pub fn load_look(themes: &Themes) -> Look {
    parse_look(&read("settings.txt"), themes)
}

pub fn save_look(themes: &Themes, look: Look) {
    write("settings.txt", &look_text(themes, look));
}

fn look_text(themes: &Themes, look: Look) -> String {
    let [front, back, felt] = [(0, look.front), (1, look.back), (2, look.felt)].map(|(k, i)| themes.name(k, i));
    format!("card_front={front}\ncard_back={back}\nfelt={felt}\n")
}

/// Looks are kept by name; one that's gone (removed from `themes.txt`) falls
/// back to the first. Older versions kept only the card back, as an index.
fn parse_look(text: &str, themes: &Themes) -> Look {
    let pick = |kind: usize, key: &str| {
        let value = text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(str::trim);
        value
            .and_then(|v| themes.find(kind, v).or_else(|| v.parse().ok().filter(|&i| i < themes.count(kind))))
            .unwrap_or(0)
    };
    Look { front: pick(0, "card_front"), back: pick(1, "card_back"), felt: pick(2, "felt") }
}

/// A session's result: initials and the net change, in half credits.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub net: i64,
}

/// The records board: the biggest wins and the biggest losses, best first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scores {
    pub wins: Vec<Entry>,
    pub losses: Vec<Entry>,
}

impl Scores {
    pub fn load() -> Self {
        Self::parse(&read("scores.txt"))
    }

    pub fn save(&self) {
        write("scores.txt", &self.to_text());
    }

    /// One entry per line: `W NAME NET` or `L NAME NET`. Bad lines are skipped.
    fn parse(text: &str) -> Self {
        let mut s = Scores::default();
        for line in text.lines() {
            let mut f = line.split_whitespace();
            let (Some(side), Some(name), Some(net)) = (f.next(), f.next(), f.next()) else { continue };
            let Ok(net) = net.parse() else { continue };
            let e = Entry { name: name.chars().take(3).collect(), net };
            match side {
                "W" if net > 0 => s.wins.push(e),
                "L" if net < 0 => s.losses.push(e),
                _ => {}
            }
        }
        s.tidy();
        s
    }

    fn to_text(&self) -> String {
        let w = self.wins.iter().map(|e| format!("W {} {}\n", e.name, e.net));
        let l = self.losses.iter().map(|e| format!("L {} {}\n", e.name, e.net));
        w.chain(l).collect()
    }

    fn tidy(&mut self) {
        self.wins.sort_by_key(|e| std::cmp::Reverse(e.net));
        self.losses.sort_by_key(|e| e.net);
        self.wins.truncate(BOARD);
        self.losses.truncate(BOARD);
    }

    /// Whether a session ending at `net` would make the board.
    pub fn is_record(&self, net: i64) -> bool {
        match net {
            n if n > 0 => self.wins.len() < BOARD || self.wins.last().is_some_and(|e| n > e.net),
            n if n < 0 => self.losses.len() < BOARD || self.losses.last().is_some_and(|e| n < e.net),
            _ => false,
        }
    }

    /// Adds a session; returns (is a win, place on that side) if it made it.
    pub fn add(&mut self, name: &str, net: i64) -> Option<(bool, usize)> {
        if !self.is_record(net) {
            return None;
        }
        let win = net > 0;
        let side = if win { &mut self.wins } else { &mut self.losses };
        // After older entries with the same result, so a tie doesn't push them down.
        let at = side.iter().position(|e| if win { net > e.net } else { net < e.net }).unwrap_or(side.len());
        side.insert(at, Entry { name: name.to_string(), net });
        side.truncate(BOARD);
        Some((win, at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn keeps_the_best_wins_and_worst_losses() {
        let mut s = Scores::default();
        for (n, net) in [("AAA", 10), ("BBB", 50), ("CCC", 30), ("DDD", 20), ("EEE", 40)] {
            assert!(s.add(n, net).is_some());
        }
        assert_eq!(names(&s.wins), ["BBB", "EEE", "CCC", "DDD", "AAA"]);
        // Full: a smaller win doesn't make it, a bigger one pushes the last out.
        assert!(!s.is_record(5));
        assert_eq!(s.add("FFF", 5), None);
        assert_eq!(s.add("GGG", 45), Some((true, 1)));
        assert_eq!(names(&s.wins), ["BBB", "GGG", "EEE", "CCC", "DDD"]);
        // Losses are their own side, worst first; breaking even is no record.
        assert_eq!(s.add("HHH", -30), Some((false, 0)));
        assert_eq!(s.add("III", -90), Some((false, 0)));
        assert_eq!(names(&s.losses), ["III", "HHH"]);
        assert!(!s.is_record(0));
    }

    #[test]
    fn looks_by_name_and_old_card_back_index() {
        let t = Themes::default();
        let look = parse_look("card_front=IVORY\ncard_back=MIDNIGHT\nfelt=WINE\n", &t);
        assert_eq!(look, Look { front: 2, back: 2, felt: 2 });
        assert_eq!(parse_look("card_back=3\n", &t), Look { front: 0, back: 3, felt: 0 });
        assert_eq!(parse_look("card_back=99\nfelt=GONE\n", &t), Look::default());
        let look = Look { front: 1, back: 4, felt: 3 };
        assert_eq!(parse_look(&look_text(&t, look), &t), look);
    }

    #[test]
    fn a_tie_goes_below_the_older_entry() {
        let mut s = Scores::default();
        s.add("OLD", 20);
        assert_eq!(s.add("NEW", 20), Some((true, 1)));
    }

    #[test]
    fn text_round_trip_and_bad_lines() {
        let mut s = Scores::default();
        s.add("ABC", 120);
        s.add("XYZ", -7);
        assert_eq!(Scores::parse(&s.to_text()), s);
        let odd = Scores::parse("W ABC notanumber\nnonsense\nW TOOLONG 5\nL WIN 10\n");
        assert_eq!(names(&odd.wins), ["TOO"]);
        assert!(odd.losses.is_empty());
    }
}
