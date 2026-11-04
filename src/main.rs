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
use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use macroquad::rand;

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

/// Sound banks, indexed by `Sfx as usize`. Decoding takes ~100 ms, so files
/// are decoded one per frame after the window is already showing the game.
struct Sounds {
    banks: [Vec<Sound>; 6],
    pending: Vec<(Sfx, &'static [u8])>,
    /// Picks which take of a sound plays. Its own generator, not the game's,
    /// so playing with sound deals the same cards as playing muted.
    pick: std::cell::Cell<u64>,
}

macro_rules! ogg {
    ($sfx:expr, $f:literal) => {
        ($sfx, include_bytes!(concat!("../assets/sfx/", $f, ".ogg")) as &[u8])
    };
}

impl Sounds {
    fn new() -> Self {
        let mut pending = vec![
            ogg!(Sfx::Shuffle, "card-shuffle"),
            ogg!(Sfx::Load, "card-fan-1"),
            ogg!(Sfx::Flip, "card-place-1"),
            ogg!(Sfx::Deal, "card-slide-1"),
            ogg!(Sfx::Chip, "chip-lay-1"),
            ogg!(Sfx::Win, "chips-stack-1"),
            ogg!(Sfx::Deal, "card-slide-2"),
            ogg!(Sfx::Deal, "card-slide-3"),
            ogg!(Sfx::Deal, "card-slide-4"),
            ogg!(Sfx::Flip, "card-place-2"),
            ogg!(Sfx::Flip, "card-place-3"),
            ogg!(Sfx::Chip, "chip-lay-2"),
            ogg!(Sfx::Win, "chips-stack-2"),
            ogg!(Sfx::Win, "chips-handle-1"),
        ];
        if std::env::var("BJ_MUTE").is_ok() {
            pending.clear();
        }
        pending.reverse();
        // Any non-zero start will do for xorshift.
        let pick = std::cell::Cell::new((miniquad::date::now() * 1e3) as u64 | 1);
        Sounds { banks: Default::default(), pending, pick }
    }

    async fn load_one(&mut self) {
        if let Some((sfx, bytes)) = self.pending.pop()
            && let Ok(s) = load_sound_from_bytes(bytes).await
        {
            self.banks[sfx as usize].push(s);
        }
    }

    fn play(&self, sfx: Sfx) {
        let bank = &self.banks[sfx as usize];
        if bank.is_empty() {
            return;
        }
        let volume = match sfx {
            Sfx::Shuffle => 0.8,
            Sfx::Chip => 0.6,
            _ => 0.7,
        };
        // xorshift
        let mut x = self.pick.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.pick.set(x);
        let i = (x % bank.len() as u64) as usize;
        play_sound(&bank[i], PlaySoundParams { looped: false, volume });
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
    };
    // BJ_NO_CRT=1 draws without the screen shader (to compare).
    let screen = if std::env::var("BJ_NO_CRT").is_ok() { None } else { shaders::screen() };

    let mut sounds = Sounds::new();

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
        game.update(dt, &input);
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

        for s in game.sfx.drain(..) {
            sounds.play(s);
        }

        // A short, soft shake (blackjack, bust), on whole pixels.
        let s = game.shake * game.shake * 2.;
        let jolt = (vec2((game.time * 61.).sin(), (game.time * 47.).cos()) * s).round();
        let mut cam = Camera2D::from_display_rect(Rect::new(-jolt.x, -jolt.y, W, H));
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        clear_background(BLACK);
        assets.draw(&game, mouse);

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
        sounds.load_one().await;
    }
}
