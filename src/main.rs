// No console window next to the game on Windows (debug builds keep it).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cards;
mod desktop;
mod game;
mod render;
mod save;
mod shaders;
mod strategy;
mod theme;

use game::{Action, Game, Input, Sfx, H, W};
use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use macroquad::rand;
use std::cell::RefCell;
use std::collections::HashMap;

fn conf() -> Conf {
    // macroquad calls this before it opens the window, so flags like
    // --install-desktop are handled here and never show the game.
    if let Some(code) = desktop::run_flags() {
        std::process::exit(code);
    }
    // Test hook: BJ_WINDOW=960x540 opens the window at that size.
    let (window_width, window_height) = std::env::var("BJ_WINDOW")
        .ok()
        .and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
        .unwrap_or((1440, 810));
    Conf {
        window_title: "Hushjack".into(),
        window_width,
        window_height,
        window_resizable: true,
        // In the browser, draw at the screen's real pixels: otherwise on a
        // scaled display the browser stretches the canvas and blurs it.
        high_dpi: cfg!(target_arch = "wasm32"),
        sample_count: 1,
        // Taskbar / Dock icon on Windows and macOS (miniquad's logo otherwise).
        icon: Some(miniquad::conf::Icon {
            small: *include_bytes!("../assets/icon/window-16.rgba"),
            medium: *include_bytes!("../assets/icon/window-32.rgba"),
            big: *include_bytes!("../assets/icon/window-64.rgba"),
        }),
        // Matches StartupWMClass in the .desktop file, so Linux desktops show its icon.
        platform: miniquad::conf::Platform { linux_wm_class: "hushjack", ..Default::default() },
        ..Default::default()
    }
}

/// Sound banks, indexed by `Sfx as usize`: the takes of each sound (see
/// assets/sfx/build.sh). Decoding takes a while, so files are decoded one per
/// frame after the window is already showing the game, the shuffle's first.
struct Sounds {
    /// By sound and style (0 except for the click and the toggle, see
    /// `theme::CLICKS`, `theme::TOGGLES`).
    banks: HashMap<(Sfx, usize), Vec<Sound>>,
    /// The take each bank played last, so it isn't played twice in a row.
    last: RefCell<HashMap<(Sfx, usize), usize>>,
    pending: Vec<((Sfx, usize), &'static [u8])>,
    /// The chosen click and toggle styles.
    click: usize,
    toggle: usize,
    /// Picks which take plays. Its own generator, not the game's, so playing
    /// with sound deals the same cards as playing muted.
    pick: std::cell::Cell<u64>,
}

/// The takes of a bank: assets/sfx/<name>-<n>.ogg for each n.
macro_rules! bank {
    (($sfx:expr, $style:literal), $name:literal, $($n:literal)+) => {
        [$((($sfx, $style), include_bytes!(concat!("../assets/sfx/", $name, "-", $n, ".ogg")) as &[u8])),+]
    };
    ($sfx:expr, $name:literal, $($n:literal)+) => {
        bank!(($sfx, 0), $name, $($n)+)
    };
}

impl Sounds {
    fn new() -> Self {
        let mut pending: Vec<((Sfx, usize), &[u8])> = [
            &bank!(Sfx::Riffle, "riffle", 1 2 3 4 5 6 7)[..],
            &bank!(Sfx::Shove, "shove", 1 2 3 4 5 6 7 8),
            &bank!(Sfx::Slide, "slide", 1 2 3 4 5 6 7 8),
            &bank!(Sfx::Place, "place", 1 2 3 4 5 6 7 8 9 10 11 12),
            &bank!(Sfx::Fan, "fan", 1 2 3 4),
            &bank!(Sfx::Chip, "chip", 1 2 3 4 5 6 7),
            &bank!(Sfx::Payout, "payout", 1 2 3 4 5 6),
            &bank!(Sfx::Sweep, "sweep", 1 2 3 4 5 6),
            &bank!((Sfx::Click, 0), "click-die", 1 2 3 4),
            &bank!((Sfx::Click, 1), "click-chip", 1 2 3 4 5 6),
            &bank!((Sfx::Click, 2), "click-clack", 1 2 3 4),
            &bank!((Sfx::Toggle, 0), "toggle-fan", 1 2 3 4 5 6),
            &bank!((Sfx::Toggle, 1), "toggle-shove", 1 2 3 4),
            &bank!((Sfx::Toggle, 2), "toggle-swish", 1 2 3 4),
            &bank!(Sfx::Chime, "chime", 1 2 3 4),
        ]
        .concat();
        if std::env::var("BJ_MUTE").is_ok() {
            pending.clear();
        }
        pending.reverse();
        // Any non-zero start will do for xorshift.
        let pick = std::cell::Cell::new((miniquad::date::now() * 1e3) as u64 | 1);
        Sounds { banks: HashMap::new(), last: RefCell::default(), pending, click: 0, toggle: 0, pick }
    }

    async fn load_one(&mut self) {
        if let Some((bank, bytes)) = self.pending.pop()
            && let Ok(s) = load_sound_from_bytes(bytes).await
        {
            self.banks.entry(bank).or_default().push(s);
        }
    }

    /// Plays a take of `sfx` at `volume` times its place in the mix.
    fn play(&self, sfx: Sfx, volume: f32) {
        let style = match sfx {
            Sfx::Click => self.click,
            Sfx::Toggle => self.toggle,
            _ => 0,
        };
        let Some(bank) = self.banks.get(&(sfx, style)) else { return };
        // The mix: every take is already at its bank's loudness.
        let mix = match sfx {
            Sfx::Riffle => 0.8,
            Sfx::Shove | Sfx::Slide | Sfx::Place | Sfx::Fan => 0.6,
            Sfx::Chip => 0.6,
            Sfx::Payout => 0.7,
            Sfx::Sweep => 0.5,
            Sfx::Click | Sfx::Toggle => 0.35,
            Sfx::Chime => 0.35,
        };
        // xorshift
        let mut x = self.pick.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.pick.set(x);
        let mut last = self.last.borrow_mut();
        let mut i = (x % bank.len() as u64) as usize;
        if last.get(&(sfx, style)) == Some(&i) && bank.len() > 1 {
            i = (i + 1) % bank.len();
        }
        last.insert((sfx, style), i);
        play_sound(&bank[i], PlaySoundParams { looped: false, volume: mix * volume });
    }

    /// Stops every take of these sounds that's still playing.
    fn stop(&self, sfx: &[Sfx]) {
        for take in self.banks.iter().filter(|b| sfx.contains(&b.0.0)).flat_map(|b| b.1) {
            stop_sound(take);
        }
    }
}

/// The music (see assets/music/compose.py): stems that loop together, all
/// started on the same frame so they stay in step, each faded with the run.
struct Music {
    /// Loaded so far, in `STEMS` order.
    stems: Vec<Sound>,
    pending: Vec<&'static [u8]>,
    /// Each stem's volume now, easing towards `targets`.
    volume: [f32; 5],
    playing: bool,
}

/// table, groove, lead, swing, cold
const STEMS: [&[u8]; 5] = [
    include_bytes!("../assets/music/table.ogg"),
    include_bytes!("../assets/music/groove.ogg"),
    include_bytes!("../assets/music/lead.ogg"),
    include_bytes!("../assets/music/swing.ogg"),
    include_bytes!("../assets/music/cold.ogg"),
];

/// The intro, in seconds after loading: the title card lifts from
/// `INTRO_LIFT`, and the dealer starts at `INTRO_END`.
const INTRO_LIFT: f32 = 1.5;
const INTRO_END: f32 = 2.8;

/// The music's place in the mix, under the table's sounds.
const MUSIC_VOLUME: f32 = 0.5;

impl Music {
    fn new() -> Self {
        let mut pending = STEMS.to_vec();
        if std::env::var("BJ_MUTE").is_ok() {
            pending.clear();
        }
        pending.reverse();
        Music { stems: Vec::new(), pending, volume: [0.; 5], playing: false }
    }

    async fn load_one(&mut self) {
        if let Some(bytes) = self.pending.pop()
            && let Ok(s) = load_sound_from_bytes(bytes).await
        {
            self.stems.push(s);
        }
    }

    /// How loud each stem should be: the table always, a fuller groove and
    /// then the theme as a winning run grows, the ride on a long one; on a
    /// losing run the table gives way to the cold layer.
    fn targets(g: &Game) -> [f32; 5] {
        let smooth = |a: f32, b: f32, x: f32| game::smooth((x - a) / (b - a));
        let (warm, cold) = (g.run.max(0.), (-g.run).max(0.));
        let table = 1. - smooth(0.15, 0.6, cold);
        let on = if g.look.music == 0 { MUSIC_VOLUME } else { 0. };
        [
            table,
            table * (0.55 + 0.45 * smooth(0.05, 0.3, warm)),
            table * smooth(0.2, 0.5, warm),
            smooth(0.55, 0.85, warm),
            smooth(0.15, 0.6, cold),
        ]
        .map(|v| v * on)
    }

    fn update(&mut self, g: &Game, dt: f32) {
        if self.stems.len() < STEMS.len() {
            return;
        }
        if !self.playing {
            self.playing = true;
            for s in &self.stems {
                play_sound(s, PlaySoundParams { looped: true, volume: 0. });
            }
        }
        // Fades of a second or two.
        for ((v, target), s) in self.volume.iter_mut().zip(Self::targets(g)).zip(&self.stems) {
            let next = *v + (target - *v).clamp(-dt * 0.6, dt * 0.6);
            if (next - *v).abs() > 1e-4 {
                *v = next;
                set_sound_volume(s, next);
            }
        }
    }
}

#[macroquad::main(conf)]
async fn main() {
    // A seed fixes the whole shoe (see --seed); otherwise every game is new.
    rand::srand(desktop::seed().unwrap_or((miniquad::date::now() * 1e6) as u64));

    // The card sheet, repainted with the chosen front and back (and again
    // when they change).
    let sheet = Image::from_file_with_format(include_bytes!("../assets/cards.png"), None).expect("cards.png");
    let themes = save::load_themes();
    let mut look = save::load_look(&themes);
    let paint_cards = |look: theme::Look| {
        let mut s = sheet.clone();
        render::paint_fronts(&mut s, &themes.fronts[look.front]);
        render::paint_card_back(&mut s, &themes.backs[look.back]);
        let t = Texture2D::from_image(&s);
        t.set_filter(FilterMode::Nearest);
        t
    };
    let print = |look: theme::Look| {
        let [r, g, b] = themes.felts[look.felt].print;
        Color::from_rgba(r, g, b, 191)
    };
    let font = render::Glyphs::new(include_bytes!("../assets/font.ttf"));
    let mut assets = render::Assets {
        cards: paint_cards(look),
        bg: render::make_background(&themes.felts[look.felt]),
        felt_print: print(look),
        font,
        lamp: shaders::lamp(),
        shade: shaders::shade(),
        cover: shaders::cover(),
        logo: {
            let t = Texture2D::from_rgba8(64, 64, include_bytes!("../assets/icon/window-64.rgba"));
            t.set_filter(FilterMode::Nearest);
            t
        },
    };
    // BJ_NO_CRT=1 draws without the screen shader (to compare).
    let screen = if std::env::var("BJ_NO_CRT").is_ok() { None } else { shaders::screen() };

    let mut sounds = Sounds::new();
    let mut music = Music::new();

    // Linear, so the screen shader can smooth the edges of the pixels it bends.
    // With depth, used to keep shadows from stacking (see shaders.rs).
    let rt = render_target_ex(W as u32, H as u32, RenderTargetParams { depth: true, ..Default::default() });
    rt.texture.set_filter(FilterMode::Linear);

    // Test hooks: BJ_AUTO=1 plays by itself, BJ_SPEED=n fast-forwards,
    // BJ_STACK=r,r,.. stacks the first ranks,
    // BJ_WATCH=1 lets it watch the shuffle, BJ_CHART=1 opens the strategy chart, BJ_THEMES=1 the theme picker,
    // BJ_RUN=n starts n wins in a row (-n: losses), BJ_MUTE=1 disables sound,
    // BJ_SHOTS=f1,f2,.. saves the frame (the 480x270 game, not the window) into
    // BJ_SHOT_DIR and quits after the last; frames then step a fixed 1/60 s, so a
    // recording doesn't depend on the monitor or on how slow saving is.
    // BJ_SHOT_SCREEN=1 saves the window instead, with the screen shader.
    // BJ_LEAVE_AT=n presses Esc at frame n (to see the records board).
    // BJ_INTRO=1 keeps the title card in a recording (it's skipped otherwise).
    let auto = std::env::var("BJ_AUTO").is_ok();
    let watch = std::env::var("BJ_WATCH").is_ok();
    let speed: f32 = std::env::var("BJ_SPEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1.);
    let shots: Vec<u64> = std::env::var("BJ_SHOTS")
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let shot_dir = std::env::var("BJ_SHOT_DIR").unwrap_or_else(|_| ".".into());
    let shot_screen = std::env::var("BJ_SHOT_SCREEN").is_ok();
    let leave_at: Option<u64> = std::env::var("BJ_LEAVE_AT").ok().and_then(|s| s.parse().ok());

    let mut game = Game::new();
    game.themes = themes.clone();
    game.look = look;
    game.scores = save::Scores::load();
    prevent_quit();
    game.show_chart = std::env::var("BJ_CHART").is_ok();
    game.show_themes = std::env::var("BJ_THEMES").is_ok();
    match std::env::var("BJ_RUN").ok().and_then(|s| s.parse::<i32>().ok()) {
        Some(n) if n > 0 => game.streak = n as u32,
        Some(n) => game.cold = n.unsigned_abs(),
        None => {}
    }
    // Applied once the shoe is loaded, after the cut and the burn card.
    let mut stack: Option<Vec<u8>> =
        std::env::var("BJ_STACK").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let mut fullscreen = false;
    // Seconds into the intro, counted once loading is done; recordings and
    // the auto-player skip it, so their frames stay where they were.
    let keep_intro = std::env::var("BJ_INTRO").is_ok();
    let mut intro = if (auto || !shots.is_empty()) && !keep_intro { INTRO_END } else { 0. };
    // Seconds since the game opened (the title fades in on it).
    let mut boot: f32 = 0.;
    let mut last_mouse = Vec2::ZERO;
    let mut frame: u64 = 0;

    loop {
        // Typed characters, for initials on the records board; drained every
        // frame so they don't pile up.
        let mut typed = Vec::new();
        while let Some(c) = get_char_pressed() {
            typed.push(c);
        }
        // While typing initials, letters are letters (F isn't fullscreen).
        let board = game.leaving.is_some();
        if is_key_pressed(KeyCode::F11) || !board && is_key_pressed(KeyCode::F) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }

        // Integer scale keeps pixels square; fall back to an exact fit when the
        // integer one would waste too much of the window.
        let fit = (screen_width() / W).min(screen_height() / H);
        let scale = if fit >= 1. && fit.floor() / fit >= 0.85 { fit.floor() } else { fit };
        let ox = ((screen_width() - W * scale) / 2.).floor();
        let oy = ((screen_height() - H * scale) / 2.).floor();
        // Through the same bend as the screen, so the mouse hits what it's over.
        let (mx, my) = mouse_position();
        let local = vec2((mx - ox) / (W * scale), (my - oy) / (H * scale));
        let mouse = if screen.is_some() { shaders::warp(local) } else { local } * vec2(W, H);

        let mut actions = Vec::new();
        let keys = [
            (KeyCode::H, Action::Hit),
            (KeyCode::S, Action::Stand),
            (KeyCode::D, Action::Double),
            (KeyCode::P, Action::Split),
            (KeyCode::I, Action::Insure),
            (KeyCode::N, Action::NoInsure),
            (KeyCode::C, Action::Chart),
            (KeyCode::B, Action::Theme(1, 1)),
            (KeyCode::T, Action::Themes),
            (KeyCode::M, Action::Theme(5, 1)),
            (KeyCode::Space, Action::Deal),
            (KeyCode::Enter, Action::Deal),
            (KeyCode::KpEnter, Action::Deal),
            (KeyCode::Up, Action::BetUp),
            (KeyCode::Down, Action::BetDown),
            (KeyCode::KpAdd, Action::BetUp),
            (KeyCode::KpSubtract, Action::BetDown),
            (KeyCode::Equal, Action::BetUp),
            (KeyCode::Minus, Action::BetDown),
        ];
        for (k, a) in keys {
            if is_key_pressed(k) {
                actions.push(a);
            }
        }
        let left = is_key_pressed(KeyCode::Left) || is_key_down(KeyCode::Left) && frame.is_multiple_of(3);
        let right = is_key_pressed(KeyCode::Right) || is_key_down(KeyCode::Right) && frame.is_multiple_of(3);
        if game.phase == game::Phase::Bet {
            if is_key_pressed(KeyCode::Left) {
                actions.push(Action::BetDown);
            }
            if is_key_pressed(KeyCode::Right) {
                actions.push(Action::BetUp);
            }
        }
        // Closing the window shows the records board first, like Esc.
        if is_key_pressed(KeyCode::Escape) || is_quit_requested() || leave_at == Some(frame) {
            actions.push(Action::Leave);
        }
        let mut auto_confirm = false;
        if auto && !board {
            let (a, c) = game.auto_input(watch);
            actions.extend(a);
            auto_confirm = c;
        }
        let click = is_mouse_button_pressed(MouseButton::Left);
        let confirm = click || actions.contains(&Action::Deal) || auto_confirm;
        let input = Input {
            mouse,
            mouse_moved: mouse != last_mouse,
            click,
            wheel: mouse_wheel().1,
            actions,
            confirm,
            left,
            right,
            left_tap: is_key_pressed(KeyCode::Left),
            right_tap: is_key_pressed(KeyCode::Right),
            typed,
            enter: is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter),
            backspace: is_key_pressed(KeyCode::Backspace),
        };
        last_mouse = mouse;

        if game.phase == game::Phase::Bet
            && let Some(ranks) = stack.take()
        {
            for (i, r) in ranks.into_iter().enumerate() {
                game.shoe[game.next + i] = cards::Card { rank: r, suit: (i % 4) as u8 };
            }
        }
        let dt = if shots.is_empty() { get_frame_time().min(1. / 20.) } else { 1. / 60. } * speed;
        // The game opens on its title card, which waits until every sound and
        // the music are decoded (the long stems take a moment, so play never
        // stutters), then the music starts alone, and a moment later the card
        // lifts off the table and the dealer starts. Space or a click hurries
        // it along.
        let loading = !sounds.pending.is_empty() || !music.pending.is_empty();
        if !loading && intro < INTRO_END {
            intro += dt;
            if input.confirm {
                intro = intro.max(INTRO_LIFT);
            }
        } else if !loading {
            game.update(dt, &input);
        }
        music.update(&game, dt);
        boot += dt;
        if game.scores_changed {
            game.scores_changed = false;
            game.scores.save();
        }
        // A browser tab can't be closed from the page, so leaving there sits
        // down at a fresh table instead (same looks and records).
        if game.quit && cfg!(target_arch = "wasm32") {
            let scores = std::mem::take(&mut game.scores);
            game = Game::new();
            game.themes = themes.clone();
            game.look = look;
            game.scores = scores;
        } else if game.quit {
            break;
        }
        if game.look != look {
            if game.look.front != look.front || game.look.back != look.back {
                assets.cards = paint_cards(game.look);
            }
            if game.look.felt != look.felt {
                assets.bg = render::make_background(&themes.felts[game.look.felt]);
                assets.felt_print = print(game.look);
            }
            look = game.look;
            save::save_look(&themes, look);
        }

        if game.hush_shuffle {
            game.hush_shuffle = false;
            sounds.stop(&game::SHUFFLE_SFX);
        }
        sounds.click = game.look.click;
        sounds.toggle = game.look.toggle;
        for (s, volume) in game.sfx.drain(..) {
            sounds.play(s, volume);
        }

        // A short, soft shake (blackjack, bust), on whole pixels.
        let s = game.shake * game.shake * 2.;
        let jolt = (vec2((game.time * 61.).sin(), (game.time * 47.).cos()) * s).round();
        let mut cam = Camera2D::from_display_rect(Rect::new(-jolt.x, -jolt.y, W, H));
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        clear_background(BLACK);
        assets.draw(&game, mouse);
        assets.intro(boot, loading, (intro - INTRO_LIFT) / (INTRO_END - INTRO_LIFT - 0.3));

        set_default_camera();
        let border = Color::from_rgba(10, 8, 14, 255);
        clear_background(border);
        if let Some(screen) = &screen {
            screen.set_uniform("src_size", vec2(W, H));
            screen.set_uniform("dst_size", vec2(W * scale, H * scale));
            screen.set_uniform("border", vec3(border.r, border.g, border.b));
            screen.set_uniform("time", game.time);
            gl_use_material(screen);
        } else {
            rt.texture.set_filter(FilterMode::Nearest);
        }
        draw_texture_ex(
            &rt.texture,
            ox,
            oy,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(W * scale, H * scale)), flip_y: true, ..Default::default() },
        );
        gl_use_default_material();

        frame += 1;
        if shots.contains(&frame) {
            let image = if shot_screen { get_screen_data() } else { rt.texture.get_texture_data() };
            image.export_png(&format!("{shot_dir}/shot_{frame:05}.png"));
            if shots.iter().max() == Some(&frame) {
                break;
            }
        }
        next_frame().await;
        // While loading, as much as fits in a few frames' time.
        let start = miniquad::date::now();
        while miniquad::date::now() - start < 0.05 && !sounds.pending.is_empty() {
            sounds.load_one().await;
        }
        if sounds.pending.is_empty() {
            music.load_one().await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_sound_file_is_in_a_bank() {
        let files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sfx"))
            .unwrap()
            .filter(|f| f.as_ref().unwrap().path().extension().is_some_and(|e| e == "ogg"))
            .count();
        assert_eq!(super::Sounds::new().pending.len(), files);
    }

    #[test]
    fn every_music_stem_is_played() {
        let files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/music"))
            .unwrap()
            .filter(|f| f.as_ref().unwrap().path().extension().is_some_and(|e| e == "ogg"))
            .count();
        assert_eq!(super::STEMS.len(), files);
    }
}
