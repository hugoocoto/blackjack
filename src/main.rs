// No console window next to the game on Windows (debug builds keep it).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cards;
mod desktop;
mod game;
mod render;
mod strategy;

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
    Conf {
        window_title: "Hushjack".into(),
        window_width: 1440,
        window_height: 810,
        window_resizable: true,
        high_dpi: false,
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
        Sounds { banks: Default::default(), pending }
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
        let i = rand::gen_range(0, bank.len());
        play_sound(&bank[i], PlaySoundParams { looped: false, volume });
    }
}

#[macroquad::main(conf)]
async fn main() {
    rand::srand((miniquad::date::now() * 1e6) as u64);

    let cards = Texture2D::from_file_with_format(include_bytes!("../assets/cards.png"), None);
    cards.set_filter(FilterMode::Nearest);
    let font = render::Glyphs::new(include_bytes!("../assets/font.ttf"));
    let assets = render::Assets { cards, bg: render::make_background(), font };

    let mut sounds = Sounds::new();

    let rt = render_target(W as u32, H as u32);
    rt.texture.set_filter(FilterMode::Nearest);
    let mut cam = Camera2D::from_display_rect(Rect::new(0., 0., W, H));
    cam.render_target = Some(rt.clone());

    // Test hooks: BJ_AUTO=1 plays by itself, BJ_SPEED=n fast-forwards,
    // BJ_STACK=r,r,.. stacks the first ranks,
    // BJ_WATCH=1 lets it watch the shuffle, BJ_CHART=1 opens the strategy chart, BJ_MUTE=1 disables sound,
    // BJ_SHOTS=f1,f2,.. saves the frame into BJ_SHOT_DIR and quits after the last.
    let auto = std::env::var("BJ_AUTO").is_ok();
    let watch = std::env::var("BJ_WATCH").is_ok();
    let speed: f32 = std::env::var("BJ_SPEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1.);
    let shots: Vec<u64> = std::env::var("BJ_SHOTS")
        .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let shot_dir = std::env::var("BJ_SHOT_DIR").unwrap_or_else(|_| ".".into());

    let mut game = Game::new();
    game.show_chart = std::env::var("BJ_CHART").is_ok();
    // Applied once the shoe is loaded, after the cut and the burn card.
    let mut stack: Option<Vec<u8>> =
        std::env::var("BJ_STACK").ok().map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect());
    let mut fullscreen = false;
    let mut last_mouse = Vec2::ZERO;
    let mut frame: u64 = 0;

    loop {
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        if is_key_pressed(KeyCode::F11) || is_key_pressed(KeyCode::F) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }

        // Integer scale keeps pixels square; fall back to an exact fit when the
        // integer one would waste too much of the window.
        let fit = (screen_width() / W).min(screen_height() / H);
        let scale = if fit >= 1. && fit.floor() / fit >= 0.85 { fit.floor() } else { fit };
        let ox = ((screen_width() - W * scale) / 2.).floor();
        let oy = ((screen_height() - H * scale) / 2.).floor();
        let (mx, my) = mouse_position();
        let mouse = vec2((mx - ox) / scale, (my - oy) / scale);

        let mut actions = Vec::new();
        let keys = [
            (KeyCode::H, Action::Hit),
            (KeyCode::S, Action::Stand),
            (KeyCode::D, Action::Double),
            (KeyCode::P, Action::Split),
            (KeyCode::I, Action::Insure),
            (KeyCode::N, Action::NoInsure),
            (KeyCode::C, Action::Chart),
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
        if auto {
            actions.extend(game.auto_action());
        }
        let click = is_mouse_button_pressed(MouseButton::Left);
        let auto_skip = auto && game.t > 0.6 && !(watch && game.phase == game::Phase::Shuffle);
        let confirm = click || actions.contains(&Action::Deal) || auto_skip;
        let input = Input {
            mouse,
            mouse_moved: mouse != last_mouse,
            click,
            wheel: mouse_wheel().1,
            actions,
            confirm,
            left,
            right,
        };
        last_mouse = mouse;

        if game.phase == game::Phase::Bet
            && let Some(ranks) = stack.take()
        {
            for (i, r) in ranks.into_iter().enumerate() {
                game.shoe[game.next + i] = cards::Card { rank: r, suit: (i % 4) as u8 };
            }
        }
        let dt = get_frame_time().min(1. / 20.) * speed;
        game.update(dt, &input);

        for s in game.sfx.drain(..) {
            sounds.play(s);
        }

        set_camera(&cam);
        clear_background(BLACK);
        assets.draw(&game, mouse);

        set_default_camera();
        clear_background(Color::from_rgba(10, 8, 14, 255));
        draw_texture_ex(
            &rt.texture,
            ox,
            oy,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(W * scale, H * scale)), flip_y: true, ..Default::default() },
        );

        frame += 1;
        if shots.contains(&frame) {
            get_screen_data().export_png(&format!("{shot_dir}/shot_{frame:05}.png"));
            if shots.iter().max() == Some(&frame) {
                break;
            }
        }
        next_frame().await;
        sounds.load_one().await;
    }
}
