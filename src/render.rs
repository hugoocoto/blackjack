use crate::cards::Card;
use crate::game::*;
use macroquad::prelude::*;
use std::f32::consts::PI;

pub struct Assets {
    pub cards: Texture2D,
    pub bg: Texture2D,
    pub font: Glyphs,
}

struct Glyph {
    src: Rect,
    /// Offset of the bitmap's top-left from the pen position on the baseline.
    off: Vec2,
    advance: f32,
}

/// The pixel font rasterized once at its native size, with every pixel either
/// fully on or off. Its cut corners otherwise come out half covered, which
/// shows up as grey "shadow" pixels in the corners of the glyphs.
pub struct Glyphs {
    tex: Texture2D,
    map: std::collections::HashMap<char, Glyph>,
}

impl Glyphs {
    pub fn new(ttf: &[u8]) -> Self {
        let font = fontdue::Font::from_bytes(ttf, fontdue::FontSettings::default()).unwrap();
        let raster: Vec<(char, fontdue::Metrics, Vec<u8>)> = (' '..='~')
            .map(|c| {
                let (m, bmp) = font.rasterize(c, SMALL as f32);
                (c, m, bmp)
            })
            .collect();
        let w: usize = raster.iter().map(|r| r.1.width + 1).sum();
        let h = raster.iter().map(|r| r.1.height).max().unwrap_or(1);
        let mut img = Image::gen_image_color(w.max(1) as u16, h.max(1) as u16, Color::new(1., 1., 1., 0.));
        let mut map = std::collections::HashMap::new();
        let mut x0 = 0;
        for (c, m, bmp) in raster {
            // Half-covered pixels come from diagonal cuts. One that fills the
            // corner between two solid pixels is a rounded corner: leave it off,
            // like the font's other corners. Otherwise it carries a diagonal
            // stroke (the 7): keep it.
            let at = |x: i32, y: i32| {
                let inside = x >= 0 && y >= 0 && (x as usize) < m.width && (y as usize) < m.height;
                if inside { bmp[y as usize * m.width + x as usize] } else { 0 }
            };
            for y in 0..m.height as i32 {
                for x in 0..m.width as i32 {
                    let v = at(x, y);
                    let solid = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dy)| at(x + dx, y + dy) >= 192).count();
                    if v >= 192 || (v >= 64 && solid < 2) {
                        img.set_pixel((x0 as i32 + x) as u32, y as u32, WHITE);
                    }
                }
            }
            let glyph = Glyph {
                src: Rect::new(x0 as f32, 0., m.width as f32, m.height as f32),
                off: vec2(m.xmin as f32, -(m.ymin + m.height as i32) as f32),
                advance: m.advance_width.round(),
            };
            map.insert(c, glyph);
            x0 += m.width + 1;
        }
        let tex = Texture2D::from_image(&img);
        tex.set_filter(FilterMode::Nearest);
        Glyphs { tex, map }
    }
}

const SHADOW: Color = Color::new(0., 0., 0., 0.35);
const INK: Color = Color::new(0.07, 0.06, 0.1, 1.);
const WHITE_T: Color = Color::new(0.96, 0.95, 0.91, 1.);
const MUTED: Color = Color::new(0.62, 0.66, 0.72, 1.);
const GOLD: Color = Color::new(0.95, 0.76, 0.3, 1.);
const RED_T: Color = Color::new(0.93, 0.35, 0.35, 1.);
const CUT_YELLOW: Color = Color::new(0.98, 0.82, 0.2, 1.);
const CUT_DARK: Color = Color::new(0.6, 0.42, 0.08, 1.);
const FELT_TEXT: Color = Color::new(0.45, 0.72, 0.58, 0.75);
const PILL_BG: Color = Color::new(0.09, 0.09, 0.13, 0.9);
const EDGE_A: Color = Color::new(0.91, 0.89, 0.85, 1.);
const EDGE_B: Color = Color::new(0.77, 0.78, 0.82, 1.);

/// Small text is the font's native size; big is exactly 2x so pixels stay square.
pub const SMALL: u16 = 16;
pub const BIG: u16 = 32;

const HUD_Y: f32 = 226.;
/// The table's arcs are ellipses centred at (W / 2, TABLE_CY).
const TABLE_CY: f32 = -6.;

fn ease(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t) * (1. - t) * (1. - t)
}

fn layers(n: usize) -> f32 {
    (n / 12).clamp(1, 26) as f32
}

impl Assets {
    fn advance(&self, c: char) -> f32 {
        self.font.map.get(&c).map_or(0., |g| g.advance)
    }

    pub fn text_width(&self, s: &str, size: u16) -> f32 {
        let k = (size / SMALL).max(1) as f32;
        s.chars().map(|c| self.advance(c)).sum::<f32>() * k
    }

    /// Glyphs are rasterized at the font's native size and placed on whole
    /// pixels; bigger sizes are integer scales of it, so the text stays pixel-exact.
    pub fn text(&self, s: &str, x: f32, y: f32, size: u16, color: Color, align: f32) {
        let k = (size / SMALL).max(1) as f32;
        let x0 = (x - self.text_width(s, size) * align).round();
        let y = y.round();
        let shadow = Color::new(0., 0., 0., 0.55 * color.a);
        // Felt print gets no drop shadow.
        let passes: &[(f32, Color)] = if color == FELT_TEXT { &[(0., color)] } else { &[(k, shadow), (0., color)] };
        for &(dy, c) in passes {
            let mut cx = x0;
            for ch in s.chars() {
                let Some(g) = self.font.map.get(&ch) else { continue };
                if g.src.w > 0. {
                    let p = DrawTextureParams {
                        source: Some(g.src),
                        dest_size: Some(vec2(g.src.w * k, g.src.h * k)),
                        ..Default::default()
                    };
                    draw_texture_ex(&self.font.tex, cx + g.off.x * k, y + dy + g.off.y * k, c, p);
                }
                cx += g.advance * k;
            }
        }
    }

    fn card_src(card: Option<Card>) -> Rect {
        match card {
            Some(c) => Rect::new((c.rank as f32 - 1.) * 64. + 11., c.suit as f32 * 64. + 2., CW, CH),
            None => Rect::new(13. * 64. + 11., 64. + 2., CW, CH),
        }
    }

    /// `squash` is the horizontal scale used by the flip animation; `rot` turns
    /// the card around its centre.
    fn card(&self, card: Option<Card>, pos: Vec2, squash: f32, rot: f32, shadow: bool) {
        let w = (CW * squash).max(2.);
        let x = (pos.x + (CW - w) / 2.).round();
        let y = pos.y.round();
        let p = |c: Option<Card>| DrawTextureParams {
            source: Some(Self::card_src(c)),
            dest_size: Some(vec2(w.round(), CH)),
            rotation: rot,
            ..Default::default()
        };
        if shadow {
            draw_texture_ex(&self.cards, x + 2., y + 3., SHADOW, p(None));
        }
        draw_texture_ex(&self.cards, x, y, WHITE, p(card));
    }

    fn sprite(&self, s: &Sprite) {
        let face = s.flip > 0.5;
        let squash = (1. - 2. * s.flip).abs();
        self.card(if face { Some(s.card) } else { None }, s.pos, squash, s.rot, true);
    }

    /// A face-down card drawn around its centre.
    fn card_at(&self, card: Option<Card>, c: Vec2, rot: f32, tint: Color) {
        let p = DrawTextureParams { source: Some(Self::card_src(card)), rotation: rot, ..Default::default() };
        draw_texture_ex(&self.cards, (c.x - CW / 2.).round(), (c.y - CH / 2.).round(), tint, p);
    }

    /// A face-down stack around centre `c`: card edges below, top card on top.
    /// A loose stack has its cards not squared.
    fn pile_at(&self, c: Vec2, rot: f32, n: usize, loose: bool) {
        if n == 0 {
            return;
        }
        let l = layers(n) as i32;
        let c = vec2(c.x.round(), c.y.round());
        let jig = |i: i32| if loose { ((i * 7 + 3) % 3 - 1) as f32 } else { 0. };
        let rect = |p: Vec2, w: f32, h: f32, color: Color| {
            draw_rectangle_ex(p.x, p.y, w, h, DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: rot, color });
        };
        self.card_at(None, c + vec2(2., 3.), rot, SHADOW);
        let outline = Color::from_rgba(70, 76, 96, 255);
        rect(c, CW, CH, outline);
        for i in 0..l {
            let color = if i % 2 == 0 { EDGE_A } else { EDGE_B };
            rect(c + vec2(jig(i), -i as f32), CW - 2., CH - 1., color);
        }
        self.card_at(None, c + vec2(jig(l), -l as f32), rot, WHITE);
    }

    fn pile(&self, pos: Vec2, n: usize) {
        self.pile_at(pos + vec2(CW / 2., CH / 2.), 0., n, false);
    }

    fn cut_card(&self, pos: Vec2, w: f32, h: f32) {
        let (x, y) = (pos.x.round(), pos.y.round());
        draw_rectangle(x + 1., y + 2., w, h, SHADOW);
        draw_rectangle(x, y, w, h, CUT_DARK);
        draw_rectangle(x + 1., y + 1., w - 2., h - 2., CUT_YELLOW);
    }

    fn chip(&self, x: f32, y: f32, color: Color) {
        let dark = Color::new(color.r * 0.6, color.g * 0.6, color.b * 0.6, 1.);
        draw_rectangle(x + 2., y + 1., 10., 5., dark);
        draw_rectangle(x, y + 2., 14., 3., dark);
        draw_rectangle(x + 2., y, 10., 4., color);
        draw_rectangle(x, y + 1., 14., 2., color);
        draw_rectangle(x + 6., y, 2., 1., WHITE_T);
        draw_rectangle(x, y + 2., 1., 1., WHITE_T);
        draw_rectangle(x + 13., y + 2., 1., 1., WHITE_T);
    }

    /// Chips for `amount` half credits, stacked upward from (x, y).
    fn chip_stack(&self, x: f32, y: f32, amount: i64) {
        const DENOMS: [(i64, Color); 6] = [
            (2000, Color::new(0.9, 0.7, 0.24, 1.)),
            (1000, Color::new(0.55, 0.32, 0.78, 1.)),
            (200, Color::new(0.2, 0.2, 0.25, 1.)),
            (50, Color::new(0.31, 0.68, 0.36, 1.)),
            (10, Color::new(0.84, 0.27, 0.27, 1.)),
            (1, Color::new(0.88, 0.88, 0.88, 1.)),
        ];
        let mut left = amount;
        let mut chips = Vec::new();
        for (v, c) in DENOMS {
            while left >= v && chips.len() < 8 {
                left -= v;
                chips.push(c);
            }
        }
        draw_rectangle(x + 1., y + 5., 13., 2., SHADOW);
        for (i, c) in chips.iter().enumerate() {
            self.chip(x, y - i as f32 * 2., *c);
        }
    }

    fn pill(&self, s: &str, cx: f32, y: f32, fg: Color, bg: Color) {
        let w = self.text_width(s, SMALL) + 10.;
        let x = (cx - w / 2.).round();
        draw_rectangle(x, y + 1., w, 14., Color::new(0., 0., 0., 0.4));
        draw_rectangle(x, y, w, 13., bg);
        self.text(s, cx, y + 10., SMALL, fg, 0.5);
    }

    // ------------------------------------------------------------ scene parts

    /// Top view of an open shoe: the stack stands on edge, front card at the
    /// mouth (left), the weight pushing it from behind, the cut card sticking out.
    fn shoe(&self, g: &Game) {
        let b = g.shoe_body();
        let s = Rect::new(b.x + SHOE_WELL.x, b.y + SHOE_WELL.y, SHOE_WELL.w, SHOE_WELL.h);
        let home = matches!(g.phase, Phase::Bet | Phase::Round);
        let loaded = home || (g.phase == Phase::CutAnim && g.t >= CA_LOAD);
        if home && g.remaining() > 0 {
            // The front card, half out of the mouth. A dealt card starts exactly
            // here, so it is this card being pulled out; the next one then
            // slides forward from under the mouth plate.
            let k = smooth((g.since_deal - 0.12) / 0.25);
            let hidden = SHOE_BODY.x - (SHOE_MOUTH.x + CW / 2. - CH / 2.);
            self.card(None, SHOE_MOUTH + vec2(((1. - k) * hidden).round(), 0.), 1., PI / 2., true);
        }
        draw_rectangle(b.x + 2., b.y + 3., b.w, b.h, SHADOW);
        draw_rectangle(b.x, b.y, b.w, b.h, Color::from_rgba(12, 12, 18, 255));
        draw_rectangle(b.x + 1., b.y + 1., b.w - 2., b.h - 2., Color::from_rgba(40, 40, 54, 255));
        draw_rectangle(b.x + 1., b.y + 1., b.w - 2., 1., Color::from_rgba(84, 84, 104, 255));
        // Well the cards stand in.
        draw_rectangle(s.x - 2., s.y - 1., s.w + 10., s.h + 2., Color::from_rgba(18, 18, 26, 255));
        // Mouth plate, slanted towards the table.
        draw_rectangle(b.x + 1., b.y + 1., 10., b.h - 2., Color::from_rgba(58, 58, 76, 255));
        for i in (0..((b.h - 2.) as i32)).step_by(6) {
            let dx = if i % 12 == 0 { 3. } else { 0. };
            draw_rectangle(b.x + 2. + dx, b.y + 1. + i as f32, 4., 1., Color::from_rgba(92, 92, 116, 255));
        }
        draw_rectangle(b.x + 11., b.y + 1., 1., b.h - 2., Color::from_rgba(12, 12, 18, 255));

        let len = if loaded && g.remaining() > 0 {
            (s.w * g.remaining() as f32 / SHOE_CARDS as f32).round().max(1.)
        } else {
            0.
        };
        for i in 0..len as i32 {
            let c = if i % 2 == 0 { EDGE_A } else { EDGE_B };
            draw_rectangle(s.x + i as f32, s.y, 1., s.h, c);
        }
        if len > 0. {
            draw_rectangle(s.x, s.y, len, 1., Color::from_rgba(170, 168, 160, 255));
            draw_rectangle(s.x, s.y + s.h - 1., len, 1., Color::from_rgba(150, 148, 140, 255));
            if !g.cut_out && g.cut > g.next {
                let cx = s.x + (s.w * (g.cut - g.next) as f32 / SHOE_CARDS as f32).round();
                self.cut_card(vec2(cx, s.y - 4.), 2., s.h + 8.);
            }
        }
        if len > 0. {
            // Cut card covering the bottom card.
            self.cut_card(vec2(s.x + len, s.y - 2.), 2., s.h + 4.);
        }
        // Weight block behind the stack.
        let wx = s.x + len + if len > 0. { 2. } else { 0. };
        draw_rectangle(wx, s.y - 1., 7., s.h + 2., Color::from_rgba(96, 96, 116, 255));
        draw_rectangle(wx + 1., s.y, 5., s.h, Color::from_rgba(70, 70, 88, 255));
        draw_rectangle(wx + 2., s.y + s.h / 2. - 5., 3., 10., Color::from_rgba(140, 140, 160, 255));
    }

    fn discard(&self, g: &Game) {
        let p = DISCARD_POS - vec2(3., 3.);
        draw_rectangle(p.x, p.y, CW + 6., CH + 6., Color::new(0., 0., 0., 0.18));
        self.pile(DISCARD_POS, g.discard);
        if g.discard == 0 {
            self.text("DISCARD", DISCARD_POS.x + CW / 2., DISCARD_POS.y + 34., SMALL, FELT_TEXT, 0.5);
        }
    }

    /// Felt print that follows an arc of the table (an ellipse centred like the
    /// table, half-axes `a` and `b`). Glyphs stay upright and step along the
    /// curve a pixel at a time; `drop` moves the baseline below the curve.
    fn arc_text(&self, s: &str, centre_x: f32, a: f32, b: f32, drop: f32) {
        let mut x = (centre_x - self.text_width(s, SMALL) / 2.).round();
        for ch in s.chars() {
            let Some(g) = self.font.map.get(&ch) else { continue };
            let dx = (x + g.advance / 2. - W / 2.) / a;
            let y = (TABLE_CY + b * (1. - dx * dx).max(0.).sqrt() + drop).round();
            if g.src.w > 0. {
                let p = DrawTextureParams { source: Some(g.src), ..Default::default() };
                draw_texture_ex(&self.font.tex, x + g.off.x, y + g.off.y, FELT_TEXT, p);
            }
            x += g.advance;
        }
    }

    fn table_text(&self) {
        // Inside the insurance band, above it, and split along its outside.
        self.arc_text("INSURANCE PAYS 2 TO 1", W / 2., 179., 103., 3.);
        self.arc_text("BLACKJACK PAYS 3 TO 2", W / 2., 168., 89., 0.);
        self.arc_text("DEALER STANDS", W / 2. - 112., 202., 120., 7.);
        self.arc_text("ON ALL 17", W / 2. + 112., 202., 120., 7.);
    }

    fn riffle_flyers(&self, from: [(Vec2, usize); 2], to: (Vec2, usize), v: f32) {
        let top = |p: Vec2, n: usize| p - vec2(0., if n == 0 { 0. } else { layers(n) });
        for j in 0..3 {
            let ph = v * 9. + j as f32 * 0.33;
            let f = ph.fract();
            let (p, n) = from[(ph as usize + j) % 2];
            let pos = top(p, n).lerp(top(to.0, to.1) - vec2(0., 2.), f) - vec2(0., (f * PI).sin() * 8.);
            self.card_at(None, pos + vec2(2., 3.), 0., SHADOW);
            self.card_at(None, pos, 0., WHITE);
        }
    }

    /// The two plastic cut cards the dealer lays in the middle.
    fn cut_cards(&self, spot: Spot, n: usize) {
        for i in 0..n {
            let o = if i == 0 { vec2(-3., 3.) } else { vec2(3., -1.) };
            let c = spot.c + o;
            self.cut_card(vec2(c.x - CW / 2., c.y - CH / 2.), CW, CH);
        }
    }

    fn shuffle(&self, g: &Game) {
        let sp = spots();
        let v = g.shuffle_view();
        let c = v.counts;
        if v.cut_cards || matches!(v.step, Some((ShStep::CutCards, _))) {
            let n = match v.step {
                Some((ShStep::CutCards, p)) if p < 0.5 => 1,
                _ => 2,
            };
            self.cut_cards(sp[M], n);
        }
        // Piles sitting on their spots, drawn back to front so a nearer pile
        // covers a farther one. Cards in the air are drawn after them.
        let mut order: [usize; SPOTS] = std::array::from_fn(|i| i);
        order.sort_by(|&i, &j| sp[i].c.y.total_cmp(&sp[j].c.y));
        let settled = |n: [usize; SPOTS], loose: [bool; SPOTS]| {
            for i in order {
                if !(v.rolled && i == M) {
                    self.pile_at(sp[i].c, sp[i].rot, n[i], loose[i]);
                }
            }
        };
        let loose_m = |on: bool| -> [bool; SPOTS] { std::array::from_fn(|i| on && i == M) };
        match v.step {
            Some((ShStep::Gather, p)) => {
                for (from, n) in &g.gather_from {
                    self.pile_at(from.lerp(sp[M].c, ease(p)), 0., *n, false);
                }
            }
            Some((ShStep::Moves(ms), p)) => {
                let mut left = c;
                for (from, _, n) in ms {
                    left[from] -= n;
                }
                settled(left, loose_m(v.loose));
                for (from, to, n) in ms {
                    let (a, b) = (sp[from], sp[to]);
                    let e = ease(p);
                    self.pile_at(a.c.lerp(b.c, e), a.rot + (b.rot - a.rot) * e, n, false);
                }
            }
            Some((ShStep::Riffle { a, b, to } | ShStep::LooseRiffle { a, b, to }, p)) => {
                let loose = matches!(v.step, Some((ShStep::LooseRiffle { .. }, _)));
                let left_a = (c[a] as f32 * (1. - p)) as usize;
                let left_b = (c[b] as f32 * (1. - p)) as usize;
                let merged = c[to] + (c[a] - left_a) + (c[b] - left_b);
                let (mut n, mut l) = (c, loose_m(v.loose));
                (n[a], n[b], n[to]) = (left_a, left_b, merged);
                (l[a], l[b], l[to]) = (false, false, loose || v.loose);
                settled(n, l);
                self.riffle_flyers([(sp[a].c, left_a), (sp[b].c, left_b)], (sp[to].c, merged), p);
            }
            Some((ShStep::Strip { from, to }, p)) => {
                let piece = c[from].div_ceil(4);
                let k = ((p * 4.) as usize).min(3);
                let f = (p * 4.).fract();
                let landed = (k * piece).min(c[from]);
                let flying = piece.min(c[from] - landed);
                let (mut n, mut l) = (c, loose_m(v.loose));
                (n[from], n[to]) = (c[from] - landed - flying, c[to] + landed);
                (l[from], l[to]) = (false, false);
                settled(n, l);
                let lift = vec2(0., -(f * PI).sin() * 6.);
                self.pile_at(sp[from].c.lerp(sp[to].c, ease(f)) + lift, 0., flying, false);
            }
            Some((ShStep::Roll, p)) => {
                let mut n = c;
                n[M] = 0;
                settled(n, [false; SPOTS]);
                let e = ease(p);
                let from = Rect::new(sp[M].c.x - CW / 2., sp[M].c.y - CH / 2., CW, CH);
                let to = Rect::new(BAR.x, BAR.y, BAR_LOOSE_W, BAR.h);
                self.stack_bar(lerp_rect(from, to, e), None, false, 1., false);
            }
            Some((ShStep::Push, _)) | None => {
                let len = (g.shoe_body().x - 1. - BAR.x).clamp(BAR.w, BAR_LOOSE_W);
                let loose = (len - BAR.w) / (BAR_LOOSE_W - BAR.w);
                self.stack_bar(Rect::new(BAR.x, BAR.y, len, BAR.h), None, false, loose, false);
            }
            Some((ShStep::CutCards, _)) => settled(c, loose_m(v.loose)),
        }
        self.text("SHUFFLING 6 DECKS", W / 2., 200., BIG, WHITE_T, 0.5);
    }

    /// The stack seen from above while it stands on edge. `loose` (0..1) is how
    /// far its cards are from being pushed together.
    fn stack_bar(&self, r: Rect, cut: Option<usize>, highlight: bool, loose: f32, back: bool) {
        let (x, y, w, h) = (r.x.round(), r.y.round(), r.w.round(), r.h.round());
        draw_rectangle(x + 2., y + 3., w, h, SHADOW);
        let step = if w > 120. { 3. } else { 2. };
        let mut i = 0.;
        while i < w {
            let k = i as i32;
            let jig = (((k * 37 + 11) % 5 - 2) as f32 * loose).round();
            let color = if (i / step) as i32 % 2 == 0 { EDGE_A } else { EDGE_B };
            draw_rectangle(x + i, y + jig - 1., step, 1., INK);
            draw_rectangle(x + i, y + jig, step, h, color);
            draw_rectangle(x + i, y + jig + h, step, 1., INK);
            i += step;
        }
        if let Some(cut) = cut {
            let cx = (x + w * cut as f32 / SHOE_CARDS as f32).round();
            if highlight {
                draw_rectangle(x, y, cx - x, h, Color::new(0.2, 0.55, 0.9, 0.18));
            }
            let cw: f32 = if w > 120. { 3. } else { 2. };
            self.cut_card(vec2(cx - (cw / 2.).floor(), y - 6.), cw, h + 12.);
        }
        if back {
            // Second cut card, covering the bottom card.
            self.cut_card(vec2(x + w, y - 4.), 2., h + 8.);
        }
    }

    fn cut(&self, g: &Game) {
        let sp = spots();
        self.cut_cards(sp[M], 2);
        self.stack_bar(BAR, Some(g.cut_choice), true, 0., false);
        let a = Game::cut_to_x(CUT_MIN);
        let b = Game::cut_to_x(CUT_MAX);
        let dim = Color::new(0., 0., 0., 0.35);
        draw_rectangle(BAR.x, BAR.y, a - BAR.x, BAR.h, dim);
        draw_rectangle(b, BAR.y, BAR.x + BAR.w - b, BAR.h, dim);
        self.text("FRONT", BAR.x, BAR.y + BAR.h + 14., SMALL, MUTED, 0.);
        self.text("BACK", BAR.x + BAR.w, BAR.y + BAR.h + 14., SMALL, MUTED, 1.);
        self.text("CUT THE CARDS", W / 2., 162., BIG, WHITE_T, 0.5);
        let s = format!("{} CARDS GO TO THE BACK", g.cut_choice);
        self.text(&s, W / 2., 178., SMALL, GOLD, 0.5);
        self.text("MOUSE OR ARROWS TO MOVE - CLICK OR ENTER TO CUT", W / 2., 192., SMALL, MUTED, 0.5);
    }

    /// After the cut. `top` is the part drawn over the shoe (the stack going in).
    fn cut_anim(&self, g: &Game, top: bool) {
        let sp = spots();
        let t = g.t;
        let part = |a: f32, b: f32| ((t - a) / (b - a)).clamp(0., 1.);
        let slab = |x: f32| Rect::new(x - 1., BAR.y - 6., 3., BAR.h + 12.);
        let fly = |from: Rect, to: Rect, p: f32| {
            let r = lerp_rect(from, to, ease(p));
            self.cut_card(vec2(r.x, r.y), r.w.round().max(2.), r.h.round());
        };
        if top {
            if (CA_BACK..CA_LOAD).contains(&t) {
                let b = g.shoe_body();
                let well = Rect::new(b.x + SHOE_WELL.x, b.y + SHOE_WELL.y, SHOE_WELL.w, SHOE_WELL.h);
                self.stack_bar(lerp_rect(BAR, well, ease(part(CA_BACK, CA_LOAD))), Some(g.cut), false, 0., true);
            }
            return;
        }
        let middle = |i: usize| {
            let c = sp[M].c + if i == 0 { vec2(-3., 3.) } else { vec2(3., -1.) };
            Rect::new(c.x - CW / 2., c.y - CH / 2., CW, CH)
        };
        // The second cut card waits in the middle until it goes to the back.
        if t < CA_MARK {
            self.cut_cards(sp[M], 1);
        }
        let player = slab(Game::cut_to_x(g.cut_choice));
        let lifted = Rect { y: BAR.y - 34., ..player };
        let mark = slab(Game::cut_to_x(g.cut));
        let back = Rect::new(BAR.x + BAR.w, BAR.y - 4., 2., BAR.h + 8.);
        if t < CA_PULL {
            self.stack_bar(BAR, None, false, 0., false);
            if t < CA_INSERT {
                fly(middle(1), player, part(0., CA_INSERT));
            } else {
                fly(player, lifted, part(CA_INSERT, CA_PULL));
            }
        } else if t < CA_SWAP {
            // The cards in front of the cut go to the back.
            let p = smooth(part(CA_PULL, CA_SWAP));
            let fl = (BAR.w * g.cut_choice as f32 / SHOE_CARDS as f32).round();
            let rear = Rect::new(BAR.x + fl * (1. - p), BAR.y, BAR.w - fl, BAR.h);
            let front = Rect::new(BAR.x + p * (BAR.w - fl), BAR.y - (p * PI).sin() * 18., fl, BAR.h);
            self.stack_bar(rear, None, false, 0., false);
            self.stack_bar(front, None, false, 0., false);
            fly(lifted, lifted, 1.);
        } else if t < CA_MARK {
            self.stack_bar(BAR, None, false, 0., false);
            fly(lifted, mark, part(CA_SWAP, CA_MARK));
        } else if t < CA_BACK {
            self.stack_bar(BAR, Some(g.cut), false, 0., false);
            fly(middle(0), back, part(CA_MARK, CA_BACK));
        }
    }

    fn insurance(&self, g: &Game) {
        let (x, y) = (120., 96.);
        if g.insurance > 0 {
            self.chip_stack(x - 7., y, g.insurance);
            self.text(&fmt_halves(g.insurance), x + 10., y + 6., SMALL, GOLD, 0.);
            match g.insurance_paid {
                Some(true) => self.pill("INSURANCE PAID", x, y + 10., WHITE_T, Color::from_rgba(52, 150, 72, 240)),
                Some(false) => self.pill("INSURANCE LOST", x, y + 10., WHITE_T, Color::from_rgba(180, 52, 60, 240)),
                None => {}
            }
        }
        if g.phase == Phase::Round && g.insurance_offer {
            let cost = fmt_halves(g.hands[0].bet);
            let s = format!("DEALER SHOWS AN ACE - INSURANCE FOR {cost}?");
            self.pill(&s, W / 2., 91., GOLD, PILL_BG);
        }
    }

    fn hands(&self, g: &Game) {
        for s in &g.to_discard {
            self.sprite(s);
        }
        for s in &g.dealer {
            self.sprite(s);
        }
        if !g.dealer.is_empty() {
            let hidden = g.dealer.iter().any(|s| s.flip < 1.);
            let v = crate::cards::hand_value(g.dealer.iter().filter(|s| s.flip >= 1.).map(|s| s.card));
            if v.total > 0 {
                let last = Game::dealer_slot(g.dealer.len() - 1, g.dealer.len());
                let fg = if v.total > 21 { RED_T } else { WHITE_T };
                self.pill(&value_label(v, !hidden), last.x + CW + 16., DEALER_Y + 24., fg, PILL_BG);
            }
        }
        let spans = g.hand_spans();
        for (i, (h, (x, w))) in g.hands.iter().zip(spans).enumerate() {
            for s in &h.cards {
                self.sprite(s);
            }
            let cx = (x + w / 2.).round();
            let v = crate::cards::hand_value(h.cards.iter().filter(|s| s.flip >= 1.).map(|s| s.card));
            if v.total > 0 {
                let fg = if v.total > 21 { RED_T } else { WHITE_T };
                self.pill(&value_label(v, h.done), cx, HAND_Y + CH + 3., fg, PILL_BG);
            }
            self.chip_stack(cx - 17., HAND_Y + CH + 24., h.bet * 2);
            self.text(&h.bet.to_string(), cx + 1., HAND_Y + CH + 30., SMALL, GOLD, 0.);
            let active = g.phase == Phase::Round && i == g.active && !h.done && (g.player_turn || g.hands.len() > 1);
            if active {
                let r = Rect::new(x - 3., HAND_Y - 3., w + 6., CH + 6.);
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 2., GOLD);
                let bob = ((g.time * 6.).sin() * 1.5).round();
                let ty = HAND_Y - 12. + bob;
                draw_triangle(vec2(cx - 5., ty), vec2(cx + 5., ty), vec2(cx, ty + 6.), GOLD);
            }
            if let Some((o, net)) = h.outcome {
                let (label, bg) = match o {
                    Outcome::Win => ("WIN", Color::from_rgba(52, 150, 72, 240)),
                    Outcome::Blackjack => ("BLACKJACK", Color::from_rgba(200, 150, 40, 240)),
                    Outcome::Push => ("PUSH", Color::from_rgba(90, 96, 120, 240)),
                    Outcome::Lose => ("LOSE", Color::from_rgba(180, 52, 60, 240)),
                    Outcome::Bust => ("BUST", Color::from_rgba(180, 52, 60, 240)),
                };
                let s = if net > 0 {
                    format!("{label} +{}", fmt_halves(net))
                } else if net < 0 {
                    format!("{label} {}", fmt_halves(net))
                } else {
                    label.to_string()
                };
                self.pill(&s, cx, HAND_Y + 22., WHITE_T, bg);
            }
        }
    }

    /// Basic strategy chart: hard totals on the left, soft and pairs on the
    /// right, the cell for the current decision outlined.
    fn chart(&self, g: &Game) {
        use crate::strategy::*;
        let current = (g.phase == Phase::Round && g.player_turn)
            .then(|| {
                let h = g.hands.get(g.active)?;
                let can_split = g.hands.len() < 4 && !h.split_aces;
                let cards: Vec<Card> = h.cards.iter().map(|s| s.card).collect();
                lookup(&cards, g.dealer.first()?.card, can_split)
            })
            .flatten();
        let (cw, rh, lw) = (9., 8., 23.);
        // Same colours as the action buttons. "Double, else stand" is half
        // double, half stand.
        let color = |p: char| match p {
            'H' => HIT_C,
            'S' => STAND_C,
            'D' | 'd' => DOUBLE_C,
            _ => SPLIT_C,
        };
        let cell = |x: f32, y: f32, w: f32, h: f32, p: char| {
            draw_rectangle(x, y, w, h, color(p));
            if p == 'd' {
                let half = (w / 2.).floor();
                draw_rectangle(x + half, y, w - half, h, STAND_C);
            }
        };
        // Header row: the table's name, then the dealer's up card.
        let table = |x: f32, y: f32, name: &str, t: Table, rows: &[(&str, &str)]| {
            let hit = current.filter(|c| c.0 == t);
            self.text(name, x, y + 7., SMALL, WHITE_T, 0.);
            for (i, d) in DEALER.iter().enumerate() {
                let c = if hit.is_some_and(|h| h.2 == i) { GOLD } else { MUTED };
                self.text(d, x + lw + i as f32 * cw + cw / 2., y + 7., SMALL, c, 0.5);
            }
            for (r, (label, plays)) in rows.iter().enumerate() {
                let ry = y + rh * (r as f32 + 1.);
                let lc = if hit.is_some_and(|h| h.1 == r) { GOLD } else { MUTED };
                self.text(label, x, ry + 7., SMALL, lc, 0.);
                for (i, p) in plays.chars().enumerate() {
                    cell(x + lw + i as f32 * cw, ry, cw - 1., rh - 1., p);
                }
            }
            if let Some((_, r, i)) = hit {
                let (cx, ry) = (x + lw + i as f32 * cw, y + rh * (r as f32 + 1.));
                draw_rectangle_lines(cx - 2., ry - 2., cw + 3., rh + 3., 2., WHITE);
            }
        };
        let panel = |r: Rect| {
            draw_rectangle(r.x + 2., r.y + 3., r.w, r.h, SHADOW);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.06, 0.05, 0.09, 0.93));
        };
        panel(Rect::new(2., 80., 120., 120.));
        table(6., 83., "HARD", Table::Hard, &HARD);
        let legend = [('H', "HIT"), ('S', "STAND"), ('P', "SPLIT"), ('D', "DOUBLE, ELSE HIT"), ('d', "DOUBLE, ELSE STAND")];
        for (i, (k, v)) in legend.iter().enumerate() {
            let (lx, ly) = match i {
                0..=2 => ([6., 38., 80.][i], 174.),
                _ => (6., 174. + (i as f32 - 2.) * 11.),
            };
            cell(lx, ly, 7., 7., *k);
            self.text(v, lx + 10., ly + 7., SMALL, MUTED, 0.);
        }
        panel(Rect::new(358., 58., 120., 166.));
        table(362., 61., "SOFT", Table::Soft, &SOFT);
        table(362., 61. + rh * 9. + 4., "PAIR", Table::Pairs, &PAIRS);
    }

    fn hud(&self, g: &Game, mouse: Vec2) {
        // The bar sits straight on the floor under the table: no panel, no separator.
        self.text("CREDITS", 8., HUD_Y + 13., SMALL, MUTED, 0.);
        // While rolling, only show halves if the real balance has one.
        let shown = if g.credits % 2 == 0 {
            ((g.shown_credits / 2.).round() * 2.) as i64
        } else {
            g.shown_credits.round() as i64
        };
        let col = if shown < 0 { RED_T } else { WHITE_T };
        self.text(&fmt_halves(shown), 8., HUD_Y + 36., BIG, col, 0.);

        self.text("BET", 160., HUD_Y + 13., SMALL, MUTED, 0.5);
        self.text(&g.bet().to_string(), 160., HUD_Y + 36., BIG, GOLD, 0.5);

        for f in &g.floaters {
            let a = (1.6 - f.t).clamp(0., 1.);
            let c = Color::new(f.color.r, f.color.g, f.color.b, a);
            self.text(&f.text, f.pos.x, f.pos.y, BIG, c, 0.);
        }

        for b in g.buttons() {
            let hover = b.enabled && b.rect.contains(mouse);
            let base = if b.enabled { b.color } else { Color::from_rgba(56, 54, 70, 255) };
            let depth = Color::new(base.r * 0.55, base.g * 0.55, base.b * 0.55, 1.);
            let lift = if hover { 1. } else { 0. };
            let r = b.rect;
            draw_rectangle(r.x, r.y + 3., r.w, r.h - 3., depth);
            draw_rectangle(r.x, r.y - lift, r.w, r.h - 3., base);
            let fg = if b.enabled { WHITE_T } else { Color::from_rgba(110, 106, 130, 255) };
            let cy = r.y - lift + (r.h - 3.) / 2.;
            if b.key.is_empty() {
                self.text(b.label, r.x + r.w / 2., cy + 4., SMALL, fg, 0.5);
            } else {
                self.text(b.label, r.x + r.w / 2., cy + 1., SMALL, fg, 0.5);
                let kc = Color::new(fg.r, fg.g, fg.b, 0.55);
                self.text(b.key, r.x + r.w / 2., cy + 11., SMALL, kc, 0.5);
            }
        }
        let note = match g.phase {
            Phase::Bet if g.reshuffle_in.is_some() => Some("SHUFFLE TIME"),
            _ => None,
        };
        if let Some(s) = note {
            self.text(s, 410., HUD_Y + 26., SMALL, GOLD, 0.5);
        }
    }

    fn banner(&self, g: &Game) {
        let Some((s, t)) = &g.banner else { return };
        let a = t.clamp(0., 0.3) / 0.3;
        let gold = Color::new(GOLD.r, GOLD.g, GOLD.b, a);
        draw_rectangle(0., 78., W, 28., Color::new(0.05, 0.04, 0.08, 0.8 * a));
        draw_rectangle(0., 78., W, 1., gold);
        draw_rectangle(0., 105., W, 1., gold);
        self.text(s, W / 2., 98., BIG, gold, 0.5);
    }

    pub fn draw(&self, g: &Game, mouse: Vec2) {
        draw_texture(&self.bg, 0., 0., WHITE);
        self.table_text();
        self.discard(g);
        self.insurance(g);
        // Dealt cards are drawn under the shoe, so they slide out of its mouth.
        self.hands(g);
        match g.phase {
            Phase::Shuffle => self.shuffle(g),
            Phase::Cut => self.cut(g),
            Phase::CutAnim => self.cut_anim(g, false),
            _ => {}
        }
        self.shoe(g);
        if g.phase == Phase::CutAnim {
            self.cut_anim(g, true);
        }
        if g.cut_out {
            self.cut_card(g.cut_card_pos, 26., 36.);
            self.text("CUT", g.cut_card_pos.x + 13., g.cut_card_pos.y + 21., SMALL, CUT_DARK, 0.5);
        }
        if g.show_chart {
            self.chart(g);
        }
        self.hud(g, mouse);
        self.banner(g);
    }
}

fn lerp_rect(a: Rect, b: Rect, p: f32) -> Rect {
    Rect::new(a.x + (b.x - a.x) * p, a.y + (b.y - a.y) * p, a.w + (b.w - a.w) * p, a.h + (b.h - a.h) * p)
}

fn value_label(v: crate::cards::Value, done: bool) -> String {
    if v.soft && v.total < 21 && !done {
        format!("{}/{}", v.total - 10, v.total)
    } else {
        v.total.to_string()
    }
}

/// Semicircular table (flat dealer side on top, padded rail round the player
/// side) on a dark floor, with the printed lines. Generated once at startup.
pub fn make_background() -> Texture2D {
    let (w, h) = (W as u16, H as u16);
    let mut img = Image::gen_image_color(w, h, BLACK);
    let bayer = [[0., 8., 2., 10.], [12., 4., 14., 6.], [3., 11., 1., 9.], [15., 7., 13., 5.]];
    let (cx, cy, a, b, rail) = (W / 2., TABLE_CY, 258., 214., 11.);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let th = bayer[y % 4][x % 4] / 16.;
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let u = ((dx / a).powi(2) + (dy / b).powi(2)).sqrt();
            let v = ((dx / (a + rail)).powi(2) + (dy / (b + rail)).powi(2)).sqrt();
            let c: [u8; 3] = if u <= 1. {
                if y < 3 {
                    // Dealer-side edge.
                    if y == 0 { [96, 96, 116] } else { [48, 48, 62] }
                } else {
                    let ddx = (x as f32 - W / 2.) / (W / 2.);
                    let ddy = (y as f32 - 100.) / 150.;
                    let d = (ddx * ddx * 0.6 + ddy * ddy).sqrt();
                    let band = ((d * 3. + th * 0.9) as i32).clamp(0, 4);
                    let base: [[u8; 3]; 5] = [[38, 110, 80], [34, 100, 73], [30, 90, 66], [26, 79, 58], [22, 68, 50]];
                    let mut c = base[band as usize];
                    if (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) % 23 == 0 {
                        c = [c[0] + 4, c[1] + 8, c[2] + 6];
                    }
                    c
                }
            } else if v <= 1. {
                // Padded leather rail, lit from above.
                let f = (u - 1.) / ((u - 1.) + (1. - v)) + (th - 0.5) * 0.12;
                if f < 0.12 {
                    [50, 28, 20]
                } else if f < 0.35 {
                    [150, 92, 56]
                } else if f < 0.75 {
                    [118, 70, 42]
                } else {
                    [74, 42, 27]
                }
            } else if (x + y) % 4 == 0 && y % 2 == 0 {
                [30, 24, 40]
            } else {
                [22, 18, 30]
            };
            img.set_pixel(x as u32, y as u32, Color::from_rgba(c[0], c[1], c[2], 255));
        }
    }
    let line = Color::from_rgba(72, 142, 108, 255);
    let mut ellipse = |ecx: f32, ecy: f32, ea: f32, eb: f32, from: f32, to: f32| {
        for i in 0..3000 {
            let t = from + (to - from) * i as f32 / 3000.;
            let (x, y) = (ecx + t.cos() * ea, ecy + t.sin() * eb);
            if x >= 0. && y >= 4. && x < W && y < H {
                img.set_pixel(x as u32, y as u32, line);
            }
        }
    };
    // Insurance band.
    ellipse(cx, cy, 170., 94., PI * 0.18, PI * 0.82);
    ellipse(cx, cy, 188., 112., PI * 0.18, PI * 0.82);
    // Betting spot.
    ellipse(W / 2., 194., 26., 10., 0., PI * 2.);
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Nearest);
    tex
}
