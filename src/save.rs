//! What the game keeps between sessions, in small text files in the user's
//! data folder: the chosen card back, and the records board (the best wins
//! and the worst losses of whole sessions).

use std::path::PathBuf;
use std::{env, fs};

/// How many entries each side of the records board keeps.
pub const BOARD: usize = 5;

/// The per-user folder for the game's files, or None where there is none
/// (the web build; Android, where the app's own folder isn't wired up yet).
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
    dir().and_then(|d| fs::read_to_string(d.join(name)).ok()).unwrap_or_default()
}

fn write(name: &str, text: &str) {
    let Some(d) = dir() else { return };
    if let Err(e) = fs::create_dir_all(&d).and_then(|_| fs::write(d.join(name), text)) {
        eprintln!("hushjack: couldn't save {name}: {e}");
    }
}

/// The chosen card back (an index into `render::BACKS`).
pub fn load_card_back() -> usize {
    read("settings.txt")
        .lines()
        .find_map(|l| l.strip_prefix("card_back=")?.trim().parse().ok())
        .unwrap_or(0)
}

pub fn save_card_back(i: usize) {
    write("settings.txt", &format!("card_back={i}\n"));
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
