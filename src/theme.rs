//! How the table looks: the card fronts, the card backs and the felt. Each
//! comes built in, and more can be added in `themes.txt` next to the saved
//! records (see `Themes::add_custom` for the format). Also how its buttons
//! sound, in a few fixed styles.

pub type Rgb = [u8; 3];

/// The card faces: the paper, the light trim (frames, court figures) and the
/// ink of each suit, in sheet order (hearts, diamonds, clubs, spades).
#[derive(Clone, Debug, PartialEq)]
pub struct Front {
    pub name: String,
    pub paper: Rgb,
    pub trim: Rgb,
    pub inks: [Rgb; 4],
}

/// A card back: its fill, the woven pattern over it, the frame and mark
/// colour, and the suit in the middle (None: the sheet's own back).
#[derive(Clone, Debug, PartialEq)]
pub struct Back {
    pub name: String,
    pub fill: Rgb,
    pub weave: Rgb,
    pub accent: Rgb,
    pub suit: Option<char>,
}

/// The felt: its colour in the middle of the table (it darkens towards the
/// rail), and the colour of the lines and the words printed on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Felt {
    pub name: String,
    pub cloth: Rgb,
    pub line: Rgb,
    pub print: Rgb,
}

/// The card sheet's own colours, which the fronts replace.
pub const SHEET_PAPER: Rgb = [255, 255, 255];
pub const SHEET_TRIM: Rgb = [199, 215, 236];
pub const SHEET_RED: Rgb = [245, 44, 78];
pub const SHEET_SLATE: Rgb = [77, 87, 102];

fn lighter(c: Rgb, k: u8) -> Rgb {
    c.map(|v| v.saturating_add(k))
}

fn front(name: &str, paper: Rgb, trim: Rgb, red: Rgb, black: Rgb) -> Front {
    Front { name: name.into(), paper, trim, inks: [red, red, black, black] }
}

fn back(name: &str, fill: Rgb, accent: Rgb, suit: Option<char>) -> Back {
    Back { name: name.into(), fill, weave: lighter(fill, 14), accent, suit }
}

/// Lines and print a little lighter than the cloth.
fn felt(name: &str, cloth: Rgb) -> Felt {
    Felt { name: name.into(), cloth, line: lighter(cloth, 32), print: lighter(cloth, 74) }
}

/// Which look of each kind is chosen: indices into `Themes`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    pub front: usize,
    pub back: usize,
    pub felt: usize,
    pub click: usize,
    pub toggle: usize,
    /// 0: music on, 1: off (see `MUSIC`).
    pub music: usize,
}

impl Look {
    /// The choice in picker row `kind` (see `KINDS`).
    pub fn get(&self, kind: usize) -> usize {
        [self.front, self.back, self.felt, self.click, self.toggle, self.music][kind]
    }

    pub fn get_mut(&mut self, kind: usize) -> &mut usize {
        match kind {
            0 => &mut self.front,
            1 => &mut self.back,
            2 => &mut self.felt,
            3 => &mut self.click,
            4 => &mut self.toggle,
            _ => &mut self.music,
        }
    }
}

/// The rows of the theme picker, in order.
pub const KINDS: [&str; 6] = ["FRONT", "BACK", "FELT", "CLICK", "TOGGLE", "MUSIC"];

/// The styles of the button click and of the strategy chart and theme
/// picker toggle (assets/sfx/click-*, toggle-*, in this order); the first is
/// the default.
pub const CLICKS: [&str; 3] = ["DIE", "CHIP", "CLACK"];
pub const TOGGLES: [&str; 3] = ["FAN", "SHOVE", "SWISH"];
pub const MUSIC: [&str; 2] = ["ON", "OFF"];

#[derive(Clone, Debug, PartialEq)]
pub struct Themes {
    pub fronts: Vec<Front>,
    pub backs: Vec<Back>,
    pub felts: Vec<Felt>,
}

/// The built-in looks go with each other in any mix: the felts are muted and
/// middling, the backs clearly darker than any felt with a light frame, and
/// the fronts light, so cards read on every felt.
impl Default for Themes {
    fn default() -> Self {
        let mut four = front("FOUR COLOUR", SHEET_PAPER, SHEET_TRIM, SHEET_RED, SHEET_SLATE);
        four.inks[1] = [40, 104, 214];
        four.inks[2] = [34, 140, 64];
        Themes {
            fronts: vec![
                front("CLASSIC", SHEET_PAPER, SHEET_TRIM, SHEET_RED, SHEET_SLATE),
                four,
                front("IVORY", [246, 238, 218], [222, 206, 172], [190, 38, 52], [40, 40, 52]),
            ],
            backs: vec![
                back("PLUM", [50, 28, 66], [222, 178, 92], Some('S')),
                back("CRIMSON", [84, 18, 30], [236, 214, 170], Some('H')),
                back("MIDNIGHT", [20, 28, 58], [196, 208, 228], Some('D')),
                back("CHARCOAL", [30, 28, 36], [222, 178, 92], Some('C')),
                back("CLASSIC", [0; 3], [0; 3], None),
            ],
            felts: vec![
                Felt { line: [72, 142, 108], print: [115, 184, 148], ..felt("GREEN", [38, 110, 80]) },
                felt("BLUE", [42, 86, 124]),
                felt("WINE", [122, 50, 60]),
                felt("VIOLET", [86, 62, 116]),
                felt("SLATE", [72, 80, 90]),
            ],
        }
    }
}

impl Themes {
    /// How many looks of the picker's row `kind` there are, and the name of one.
    pub fn count(&self, kind: usize) -> usize {
        [self.fronts.len(), self.backs.len(), self.felts.len(), CLICKS.len(), TOGGLES.len(), MUSIC.len()][kind]
    }

    pub fn name(&self, kind: usize, i: usize) -> &str {
        match kind {
            0 => &self.fronts[i].name,
            1 => &self.backs[i].name,
            2 => &self.felts[i].name,
            3 => CLICKS[i],
            4 => TOGGLES[i],
            _ => MUSIC[i],
        }
    }

    /// What a new player gets: ivory cards with charcoal backs, and the first
    /// of everything else.
    pub fn default_look(&self) -> Look {
        let mut look = Look::default();
        for (kind, name) in [(0, "IVORY"), (1, "CHARCOAL")] {
            *look.get_mut(kind) = self.find(kind, name).unwrap_or(0);
        }
        look
    }

    /// The index of the look of row `kind` called `name`, if any.
    pub fn find(&self, kind: usize, name: &str) -> Option<usize> {
        (0..self.count(kind)).find(|&i| self.name(kind, i) == name)
    }

    /// Adds the looks described in `text` (a `themes.txt`), one per line:
    ///
    /// ```text
    /// # front NAME paper=R,G,B trim=R,G,B red=R,G,B black=R,G,B
    /// #       (or hearts= diamonds= clubs= spades= for one ink per suit)
    /// # back  NAME fill=R,G,B weave=R,G,B accent=R,G,B suit=S|H|D|C
    /// # felt  NAME cloth=R,G,B line=R,G,B print=R,G,B
    /// front SEPIA paper=240,228,200 trim=214,194,156 red=150,40,30 black=60,40,30
    /// felt TEAL cloth=20,96,104
    /// ```
    ///
    /// Colours left out come from the first built-in look of that kind (the
    /// felt's line and print, and the back's weave, from its main colour). In
    /// a name, `_` is a space. A look named like an existing one replaces it.
    /// Returns a message for each line it couldn't read.
    pub fn add_custom(&mut self, text: &str) -> Vec<String> {
        let mut errors = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Err(e) = self.add_line(line) {
                errors.push(format!("themes.txt line {}: {e}", n + 1));
            }
        }
        errors
    }

    fn add_line(&mut self, line: &str) -> Result<(), String> {
        let mut words = line.split_whitespace();
        let kind = words.next().unwrap_or_default();
        let name = words.next().ok_or("no name")?.replace('_', " ").to_uppercase();
        let mut keys = Vec::new();
        for w in words {
            let (k, v) = w.split_once('=').ok_or_else(|| format!("expected key=value, got {w:?}"))?;
            keys.push((k, v));
        }
        let rgb = |key: &str| -> Result<Option<Rgb>, String> {
            let Some((_, v)) = keys.iter().find(|(k, _)| *k == key) else { return Ok(None) };
            let parts: Vec<u8> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            match parts[..] {
                [r, g, b] if v.split(',').count() == 3 => Ok(Some([r, g, b])),
                _ => Err(format!("{key}={v} is not R,G,B (each 0-255)")),
            }
        };
        let known: &[&str] = match kind {
            "front" => &["paper", "trim", "red", "black", "hearts", "diamonds", "clubs", "spades"],
            "back" => &["fill", "weave", "accent", "suit"],
            "felt" => &["cloth", "line", "print"],
            _ => return Err(format!("expected front, back or felt, got {kind:?}")),
        };
        if let Some((k, _)) = keys.iter().find(|(k, _)| !known.contains(k)) {
            return Err(format!("{kind} has no {k:?}"));
        }
        match kind {
            "front" => {
                let base = &self.fronts[0];
                let mut f = Front { name, paper: base.paper, trim: base.trim, inks: base.inks };
                f.paper = rgb("paper")?.unwrap_or(f.paper);
                f.trim = rgb("trim")?.unwrap_or(f.trim);
                if let Some(c) = rgb("red")? {
                    f.inks[0] = c;
                    f.inks[1] = c;
                }
                if let Some(c) = rgb("black")? {
                    f.inks[2] = c;
                    f.inks[3] = c;
                }
                for (i, suit) in ["hearts", "diamonds", "clubs", "spades"].iter().enumerate() {
                    f.inks[i] = rgb(suit)?.unwrap_or(f.inks[i]);
                }
                upsert(&mut self.fronts, f, |f| &f.name);
            }
            "back" => {
                let base = &self.backs[0];
                let fill = rgb("fill")?.unwrap_or(base.fill);
                let suit = match keys.iter().find(|(k, _)| *k == "suit").map(|(_, v)| *v) {
                    None => base.suit,
                    Some(s @ ("S" | "H" | "D" | "C")) => s.chars().next(),
                    Some(s) => return Err(format!("suit={s} is not S, H, D or C")),
                };
                let b = Back {
                    name,
                    fill,
                    weave: rgb("weave")?.unwrap_or(lighter(fill, 14)),
                    accent: rgb("accent")?.unwrap_or(base.accent),
                    suit,
                };
                upsert(&mut self.backs, b, |b| &b.name);
            }
            _ => {
                let cloth = rgb("cloth")?.unwrap_or(self.felts[0].cloth);
                let plain = felt(&name, cloth);
                let f = Felt {
                    line: rgb("line")?.unwrap_or(plain.line),
                    print: rgb("print")?.unwrap_or(plain.print),
                    ..plain
                };
                upsert(&mut self.felts, f, |f| &f.name);
            }
        }
        Ok(())
    }
}

/// Replaces the item with the same name, or adds it at the end.
fn upsert<T>(v: &mut Vec<T>, item: T, name: impl Fn(&T) -> &String) {
    match v.iter().position(|x| name(x) == name(&item)) {
        Some(i) => v[i] = item,
        None => v.push(item),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_looks_are_added_with_defaults() {
        let mut t = Themes::default();
        let errors = t.add_custom(
            "# comment\n\
             front sepia_tone paper=240,228,200 black=60,40,30\n\
             back OCEAN fill=20,60,90 suit=D\n\
             felt TEAL cloth=20,96,104 print=1,2,3\n",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let f = t.fronts.last().unwrap();
        assert_eq!(f.name, "SEPIA TONE");
        assert_eq!(f.paper, [240, 228, 200]);
        assert_eq!(f.trim, SHEET_TRIM);
        assert_eq!(f.inks, [SHEET_RED, SHEET_RED, [60, 40, 30], [60, 40, 30]]);
        let b = t.backs.last().unwrap();
        assert_eq!((b.weave, b.suit), ([34, 74, 104], Some('D')));
        let felt = t.felts.last().unwrap();
        assert_eq!((felt.line, felt.print), ([52, 128, 136], [1, 2, 3]));
    }

    #[test]
    fn the_readme_example_reads() {
        let readme = include_str!("../README.md");
        let example = readme.split("```text\n").nth(1).unwrap().split("```").next().unwrap();
        let mut t = Themes::default();
        assert!(t.add_custom(example).is_empty());
        assert_eq!(t.fronts.last().unwrap().name, "SEPIA");
        assert_eq!(t.backs.last().unwrap().name, "OCEAN");
        assert_eq!(t.felts.last().unwrap().name, "TEAL");
    }

    #[test]
    fn same_name_replaces() {
        let mut t = Themes::default();
        let n = t.felts.len();
        assert!(t.add_custom("felt green cloth=0,80,0").is_empty());
        assert_eq!(t.felts.len(), n);
        assert_eq!(t.felts[0].cloth, [0, 80, 0]);
    }

    #[test]
    fn bad_lines_are_reported_and_skipped() {
        let mut t = Themes::default();
        let before = t.clone();
        let errors = t.add_custom(
            "table X\nfelt\nfelt A cloth=1,2\nfelt B cloth=1,2,300\nfelt C colour=1,2,3\nback D suit=X\nfelt E cloth\n",
        );
        assert_eq!(errors.len(), 7, "{errors:?}");
        assert!(errors[0].starts_with("themes.txt line 1:"));
        assert_eq!(t, before);
    }
}
