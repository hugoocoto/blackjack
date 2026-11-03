use crate::cards::*;
use macroquad::prelude::*;
use std::collections::VecDeque;
use std::f32::consts::PI;

pub const W: f32 = 480.;
pub const H: f32 = 270.;
pub const CW: f32 = 42.;
pub const CH: f32 = 60.;
pub const DECKS: usize = 6;
pub const SHOE_CARDS: usize = DECKS * 52;

/// The shoe, seen from above: body, and the area its cards stand in (on edge).
pub const SHOE_BODY: Rect = Rect { x: 384., y: 6., w: 88., h: 50. };
pub const SHOE_STACK: Rect = Rect { x: 396., y: 10., w: 64., h: 42. };
/// Top-left of the (unrotated) front card in the shoe. It lies sideways,
/// centred on the stack, so most of it is hidden under the shoe's mouth.
pub const SHOE_MOUTH: Vec2 = vec2(398. - CW / 2., 31. - CH / 2.);
pub const DISCARD_POS: Vec2 = vec2(12., 12.);
pub const CUT_CARD_REST: Vec2 = vec2(440., 62.);

pub const DEALER_Y: f32 = 12.;
pub const HAND_Y: f32 = 110.;
const DEALER_STEP: f32 = 30.;
const HAND_STEP: f32 = 16.;
const HAND_GAP: f32 = 18.;

/// The stack rolled on edge in front of the dealer, once squared by the shoe,
/// and how long it is before the shoe pushes it together.
pub const BAR: Rect = Rect { x: 150., y: 20., w: 160., h: 42. };
pub const BAR_LOOSE_W: f32 = 182.;
/// Where the shoe's body sits while it pushes the stack and takes it in.
pub const SHOE_PUSH: Vec2 = vec2(BAR.x + BAR.w + 1., BAR.y - 4.);
/// The shoe's stack area relative to its body.
pub const SHOE_WELL: Rect = Rect { x: 12., y: 4., w: 64., h: 42. };
/// The player cuts at least 10 cards in from either end.
pub const CUT_MIN: usize = 10;
pub const CUT_MAX: usize = SHOE_CARDS - 10;
/// The dealer's cut card goes about one deck from the end.
const PENETRATION: usize = SHOE_CARDS - 52;

/// After the player cuts (each value is when that part ends): the player's cut
/// card goes in; the dealer pulls it out to split the stack; the cards in
/// front of it go to the back; the same cut card goes back in about a deck
/// from the end; the second cut card covers the bottom card at the back; the
/// stack goes into the shoe; the shoe goes back to its place. Then one card
/// is burned.
pub const CA_INSERT: f32 = 0.3;
pub const CA_PULL: f32 = 0.55;
pub const CA_SWAP: f32 = 1.15;
pub const CA_MARK: f32 = 1.55;
pub const CA_BACK: f32 = 1.9;
pub const CA_LOAD: f32 = 2.4;
pub const CA_RETURN: f32 = 3.0;

const DEAL_WAIT: f32 = 0.22;
const START_CREDITS: i64 = 1000;
pub const BETS: [i64; 13] = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000];

/// Shuffle spots. L and R sit on each side of the insurance text, angled with
/// the line; M is the middle of the line; W is where the dealer works, with the
/// two riffle halves at WL / WR and the strip pile at WS; ML / MR feed the
/// final riffle into M.
pub const L: usize = 0;
pub const R: usize = 1;
pub const M: usize = 2;
pub const WK: usize = 3;
pub const WL: usize = 4;
pub const WR: usize = 5;
pub const WS: usize = 6;
pub const ML: usize = 7;
pub const MR: usize = 8;
pub const SPOTS: usize = 9;

#[derive(Clone, Copy, Debug)]
pub struct Spot {
    /// Centre of the pile and its rotation.
    pub c: Vec2,
    pub rot: f32,
}

/// A point on the middle of the insurance band, and the band's angle there.
fn band(x: f32) -> Spot {
    let (cx, cy, a, b) = (W / 2., -6., 179., 103.);
    let t = ((x - cx) / a).clamp(-1., 1.).acos();
    Spot { c: vec2(x, cy + b * t.sin()), rot: (-b * t.cos()).atan2(a * t.sin()) }
}

pub fn spots() -> [Spot; SPOTS] {
    let flat = |x: f32, y: f32| Spot { c: vec2(x, y), rot: 0. };
    [
        band(150.),
        band(330.),
        flat(240., 104.),
        flat(240., 44.),
        flat(209., 44.),
        flat(271., 44.),
        flat(182., 40.),
        flat(208., 104.),
        flat(272., 104.),
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ShStep {
    /// Every card comes to the middle.
    Gather,
    /// Up to two chunks slide at once: (from, to, cards).
    Moves([(usize, usize, usize); 2]),
    /// Two piles are riffled together onto a third.
    Riffle { a: usize, b: usize, to: usize },
    /// Same, but the cards are left half pushed in, not squared.
    LooseRiffle { a: usize, b: usize, to: usize },
    /// The top of a pile is pulled off in a few pieces onto another spot.
    Strip { from: usize, to: usize },
    /// Two cut cards are laid in the middle.
    CutCards,
    /// The middle pile is rolled on edge in front of the dealer.
    Roll,
    /// The dealer pushes the stack together with the shoe.
    Push,
}

/// Casino hand shuffle of a 6-deck shoe, as a timeline.
///
/// Split in half to both sides of the insurance line. Working stack: grab some
/// from each side, riffle, riffle, strip, riffle, and put it in the middle;
/// then keep grabbing some from one side (right, left, ...) plus some from the
/// middle, riffle-riffle-strip-riffle, back onto the middle. Split the middle
/// in half to the sides again, lay two cut cards, and riffle small groups from
/// both sides loosely onto the middle. Roll it in front of the dealer and
/// square it with the shoe.
fn shuffle_plan() -> Vec<(ShStep, f32)> {
    use ShStep::*;
    let half = SHOE_CARDS / 2;
    let mut v = vec![(Gather, 0.6), (Moves([(M, L, half), (M, R, half)]), 0.45)];
    let grab = 52;
    let mut grabs = vec![[(L, grab), (R, grab)]];
    let mut side = R;
    let mut taken = 2 * grab;
    while taken < SHOE_CARDS {
        grabs.push([(side, grab), (M, grab)]);
        side = if side == R { L } else { R };
        taken += grab;
    }
    for [(a, na), (b, nb)] in grabs {
        let n = na + nb;
        v.push((Moves([(a, WK, na), (b, WK, nb)]), 0.25));
        for strip in [false, false, true, false] {
            if strip {
                v.push((Strip { from: WK, to: WS }, 0.25));
                v.push((Moves([(WS, WK, n), (WS, WK, 0)]), 0.08));
            } else {
                v.push((Moves([(WK, WL, n / 2), (WK, WR, n - n / 2)]), 0.1));
                v.push((Riffle { a: WL, b: WR, to: WK }, 0.22));
            }
        }
        v.push((Moves([(WK, M, n), (WK, M, 0)]), 0.2));
    }
    v.push((Moves([(M, L, half), (M, R, half)]), 0.45));
    v.push((CutCards, 0.35));
    let group = 26;
    for _ in 0..half / group {
        v.push((Moves([(L, ML, group), (R, MR, group)]), 0.18));
        v.push((LooseRiffle { a: ML, b: MR, to: M }, 0.24));
    }
    v.push((Roll, 0.5));
    v.push((Push, 0.9));
    v
}

pub struct ShuffleView {
    /// Cards in each pile at the start of the current step.
    pub counts: [usize; SPOTS],
    /// Current step and its progress 0..1; None once finished.
    pub step: Option<(ShStep, f32)>,
    pub cut_cards: bool,
    pub loose: bool,
    pub rolled: bool,
}

pub struct Sprite {
    pub card: Card,
    pub pos: Vec2,
    pub face_up: bool,
    /// 0 = back showing, 1 = face showing; animates toward `face_up`.
    pub flip: f32,
    /// Lies sideways once settled (a doubled card).
    pub sideways: bool,
    /// Current rotation; cards leave the shoe sideways and turn as they travel.
    pub rot: f32,
}

impl Sprite {
    fn new(card: Card, face_up: bool) -> Self {
        Sprite { card, pos: SHOE_MOUTH, face_up, flip: 0., sideways: false, rot: PI / 2. }
    }

    fn step(&mut self, target: Vec2, dt: f32) {
        self.pos += (target - self.pos) * (1. - (-dt * 13.).exp());
        let goal = if self.sideways { PI / 2. } else { 0. };
        self.rot += (goal - self.rot) * (1. - (-dt * 11.).exp());
        if (goal - self.rot).abs() < 0.02 {
            self.rot = goal;
        }
        let goal = if self.face_up { 1. } else { 0. };
        let d = goal - self.flip;
        self.flip += d.signum() * (dt * 5.).min(d.abs());
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Outcome {
    Win,
    Lose,
    Push,
    Bust,
    Blackjack,
}

pub struct Hand {
    pub cards: Vec<Sprite>,
    /// Whole credits on this hand (already taken from `credits`).
    pub bet: i64,
    pub doubled: bool,
    pub split_aces: bool,
    pub done: bool,
    /// Outcome and net change in half credits.
    pub outcome: Option<(Outcome, i64)>,
}

impl Hand {
    fn new(bet: i64) -> Self {
        Hand { cards: Vec::new(), bet, doubled: false, split_aces: false, done: false, outcome: None }
    }
    pub fn value(&self) -> Value {
        hand_value(self.cards.iter().map(|s| s.card))
    }
    fn plain(&self) -> Vec<Card> {
        self.cards.iter().map(|s| s.card).collect()
    }
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Player(usize),
    Dealer,
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Deal(Target, bool),
    AfterDeal,
    Peek,
    Check,
    NextHand,
    DealerTurn,
    DealerDraw,
    Settle,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Shuffle,
    Cut,
    /// Cut made: swap, dealer's cut card, into the shoe, shoe back.
    CutAnim,
    Bet,
    Round,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    Hit,
    Stand,
    Double,
    Split,
    Insure,
    NoInsure,
    Deal,
    /// Show or hide the basic strategy chart.
    Chart,
    BetDown,
    BetUp,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sfx {
    Deal,
    Flip,
    Shuffle,
    Chip,
    Win,
    Load,
}

pub struct Floater {
    pub text: String,
    pub pos: Vec2,
    pub t: f32,
    pub color: Color,
}

pub struct Button {
    pub rect: Rect,
    pub label: &'static str,
    pub key: &'static str,
    pub action: Action,
    pub enabled: bool,
    pub color: Color,
}

pub struct Input {
    pub mouse: Vec2,
    pub mouse_moved: bool,
    pub click: bool,
    pub wheel: f32,
    pub actions: Vec<Action>,
    /// Space / Enter / click: confirm or skip.
    pub confirm: bool,
    pub left: bool,
    pub right: bool,
}

/// Action colours, shared by the buttons and the strategy chart.
pub const HIT_C: Color = Color::new(58. / 255., 123. / 255., 213. / 255., 1.);
pub const STAND_C: Color = Color::new(214. / 255., 69. / 255., 69. / 255., 1.);
pub const DOUBLE_C: Color = Color::new(224. / 255., 154. / 255., 44. / 255., 1.);
pub const SPLIT_C: Color = Color::new(79. / 255., 174. / 255., 92. / 255., 1.);

const FLOATER_POS: Vec2 = vec2(8., 222.);
/// Next to the insurance chips (see render.rs).
const INSURANCE_FLOATER_POS: Vec2 = vec2(70., 92.);
const GREEN_T: Color = Color::new(0.47, 0.9, 0.47, 1.);
const RED_T: Color = Color::new(0.93, 0.35, 0.35, 1.);

pub struct Game {
    pub phase: Phase,
    pub t: f32,
    pub time: f32,

    pub shoe: Vec<Card>,
    pub next: usize,
    pub cut: usize,
    pub cut_out: bool,
    pub cut_card_pos: Vec2,
    pub discard: usize,
    first_shuffle: bool,
    /// Piles that fly to the middle at the start of a shuffle: (pos, cards).
    pub gather_from: Vec<(Vec2, usize)>,
    plan: Vec<(ShStep, f32)>,
    pub cut_choice: usize,

    pub dealer: Vec<Sprite>,
    pub hands: Vec<Hand>,
    pub active: usize,
    pub to_discard: Vec<Sprite>,
    script: VecDeque<Step>,
    wait: f32,
    pub player_turn: bool,
    pub insurance_offer: bool,
    /// Half credits on insurance (= half the bet), and whether it paid.
    pub insurance: i64,
    pub insurance_paid: Option<bool>,

    /// Half credits, so 3:2 payouts stay exact.
    pub credits: i64,
    pub shown_credits: f32,
    pub bet_idx: usize,
    pub reshuffle_in: Option<f32>,

    pub floaters: Vec<Floater>,
    pub banner: Option<(String, f32)>,
    pub sfx: Vec<Sfx>,
    /// Seconds since a card left the shoe; the next card then slides into the mouth.
    pub since_deal: f32,
    pub show_chart: bool,
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

pub fn fmt_halves(v: i64) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.abs();
    if a % 2 == 0 {
        format!("{sign}{}", a / 2)
    } else {
        format!("{sign}{}.5", a / 2)
    }
}

impl Game {
    pub fn new() -> Self {
        let mut g = Game {
            phase: Phase::Shuffle,
            t: 0.,
            time: 0.,
            shoe: new_shoe(DECKS),
            next: 0,
            cut: SHOE_CARDS * 3 / 4,
            cut_out: false,
            cut_card_pos: CUT_CARD_REST,
            discard: 0,
            first_shuffle: true,
            gather_from: Vec::new(),
            plan: shuffle_plan(),
            cut_choice: SHOE_CARDS * 3 / 4,
            dealer: Vec::new(),
            hands: Vec::new(),
            active: 0,
            to_discard: Vec::new(),
            script: VecDeque::new(),
            wait: 0.,
            player_turn: false,
            insurance_offer: false,
            insurance: 0,
            insurance_paid: None,
            credits: START_CREDITS * 2,
            shown_credits: (START_CREDITS * 2) as f32,
            bet_idx: 3,
            reshuffle_in: None,
            floaters: Vec::new(),
            banner: None,
            sfx: Vec::new(),
            since_deal: 10.,
            show_chart: false,
        };
        g.begin_shuffle();
        g
    }

    pub fn bet(&self) -> i64 {
        BETS[self.bet_idx]
    }

    pub fn remaining(&self) -> usize {
        self.shoe.len() - self.next
    }

    fn floater(&mut self, net: i64) {
        self.floater_at(net, FLOATER_POS);
    }

    fn floater_at(&mut self, net: i64, pos: Vec2) {
        let (text, color) = if net > 0 {
            (format!("+{}", fmt_halves(net)), GREEN_T)
        } else if net < 0 {
            (fmt_halves(net), RED_T)
        } else {
            ("PUSH".into(), Color::new(0.9, 0.9, 0.9, 1.))
        };
        self.floaters.push(Floater { text, pos, t: 0., color });
    }

    // ---------------------------------------------------------------- shuffle

    fn begin_shuffle(&mut self) {
        self.discard += self.to_discard.len();
        self.to_discard.clear();
        self.gather_from.clear();
        if self.first_shuffle {
            // Six fresh decks laid out in a row.
            for i in 0..DECKS {
                let x = W / 2. + (i as f32 - 2.5) * 56.;
                self.gather_from.push((vec2(x, 70.), 52));
            }
        } else {
            self.gather_from.push((DISCARD_POS + vec2(CW / 2., CH / 2.), self.discard));
            self.gather_from.push((vec2(SHOE_STACK.x + 10., SHOE_STACK.y + 20.), self.remaining()));
        }
        self.first_shuffle = false;
        self.shoe = new_shoe(DECKS);
        shuffle(&mut self.shoe);
        self.next = 0;
        self.discard = 0;
        self.cut_out = false;
        self.phase = Phase::Shuffle;
        self.t = 0.;
    }

    pub fn shuffle_time(&self) -> f32 {
        self.plan.iter().map(|s| s.1).sum()
    }

    fn shuffle_step_index(&self, t: f32) -> usize {
        let mut acc = 0.;
        for (i, (_, d)) in self.plan.iter().enumerate() {
            acc += d;
            if t < acc {
                return i;
            }
        }
        self.plan.len()
    }

    pub fn shuffle_view(&self) -> ShuffleView {
        let mut v = ShuffleView { counts: [0; SPOTS], step: None, cut_cards: false, loose: false, rolled: false };
        let mut acc = 0.;
        for (step, d) in &self.plan {
            if self.t < acc + d {
                v.step = Some((*step, ((self.t - acc) / d).clamp(0., 1.)));
                return v;
            }
            acc += d;
            let c = &mut v.counts;
            match *step {
                ShStep::Gather => c[M] = SHOE_CARDS,
                ShStep::Moves(ms) => {
                    for (from, to, n) in ms {
                        c[from] -= n;
                        c[to] += n;
                    }
                }
                ShStep::Riffle { a, b, to } | ShStep::LooseRiffle { a, b, to } => {
                    c[to] += c[a] + c[b];
                    c[a] = 0;
                    c[b] = 0;
                    v.loose = matches!(step, ShStep::LooseRiffle { .. });
                }
                ShStep::Strip { from, to } => {
                    c[to] += c[from];
                    c[from] = 0;
                }
                ShStep::CutCards => v.cut_cards = true,
                ShStep::Roll => v.rolled = true,
                ShStep::Push => {}
            }
        }
        v
    }

    /// The shoe's body: at home while dealing, beside the stack while the
    /// dealer squares it, takes the cut and loads it.
    pub fn shoe_body(&self) -> Rect {
        let home = vec2(SHOE_BODY.x, SHOE_BODY.y);
        let touch = vec2(BAR.x + BAR_LOOSE_W + 1., SHOE_PUSH.y);
        let pos = match self.phase {
            Phase::Shuffle => match self.shuffle_view().step {
                Some((ShStep::Push, p)) if p < 0.5 => home.lerp(touch, smooth(p * 2.)),
                Some((ShStep::Push, p)) => touch.lerp(SHOE_PUSH, smooth(p * 2. - 1.)),
                None => SHOE_PUSH,
                _ => home,
            },
            Phase::Cut => SHOE_PUSH,
            Phase::CutAnim if self.t < CA_LOAD => SHOE_PUSH,
            Phase::CutAnim => SHOE_PUSH.lerp(home, smooth((self.t - CA_LOAD) / (CA_RETURN - CA_LOAD))),
            _ => home,
        };
        Rect::new(pos.x.round(), pos.y.round(), SHOE_BODY.w, SHOE_BODY.h)
    }

    pub fn cut_from_x(x: f32) -> usize {
        let f = ((x - BAR.x) / BAR.w).clamp(0., 1.);
        ((f * SHOE_CARDS as f32).round() as usize).clamp(CUT_MIN, CUT_MAX)
    }

    pub fn cut_to_x(i: usize) -> f32 {
        BAR.x + BAR.w * i as f32 / SHOE_CARDS as f32
    }

    // ---------------------------------------------------------------- layout

    pub fn dealer_slot(i: usize, n: usize) -> Vec2 {
        let total = CW + (n.max(1) - 1) as f32 * DEALER_STEP;
        vec2(W / 2. - total / 2. + i as f32 * DEALER_STEP, DEALER_Y)
    }

    fn hand_width(h: &Hand) -> f32 {
        let n = h.cards.len().max(1);
        let last_w = if h.cards.last().is_some_and(|s| s.sideways) { CH } else { CW };
        (n - 1) as f32 * HAND_STEP + last_w
    }

    /// Left x and width of every player hand.
    pub fn hand_spans(&self) -> Vec<(f32, f32)> {
        let widths: Vec<f32> = self.hands.iter().map(Self::hand_width).collect();
        let total: f32 = widths.iter().sum::<f32>() + HAND_GAP * (widths.len().max(1) - 1) as f32;
        let mut x = W / 2. - total / 2.;
        widths
            .into_iter()
            .map(|w| {
                let s = (x, w);
                x += w + HAND_GAP;
                s
            })
            .collect()
    }

    fn hand_slot(x: f32, i: usize, sideways: bool) -> Vec2 {
        let cx = if sideways { i as f32 * HAND_STEP + CH / 2. } else { i as f32 * HAND_STEP + CW / 2. };
        vec2(x + cx - CW / 2., HAND_Y)
    }

    // ---------------------------------------------------------------- buttons

    pub fn buttons(&self) -> Vec<Button> {
        let mut b = Vec::new();
        let betting = self.phase == Phase::Bet && self.reshuffle_in.is_none();
        let grey = Color::from_rgba(90, 96, 120, 255);
        b.push(Button {
            rect: Rect::new(116., 236., 18., 24.),
            label: "<",
            key: "",
            action: Action::BetDown,
            enabled: betting && self.bet_idx > 0,
            color: grey,
        });
        b.push(Button {
            rect: Rect::new(186., 236., 18., 24.),
            label: ">",
            key: "",
            action: Action::BetUp,
            enabled: betting && self.bet_idx + 1 < BETS.len(),
            color: grey,
        });
        b.push(Button {
            rect: Rect::new(92., 236., 18., 24.),
            label: "?",
            key: "",
            action: Action::Chart,
            enabled: true,
            color: if self.show_chart { Color::from_rgba(200, 150, 40, 255) } else { grey },
        });
        let slot = |i: usize| Rect::new(214. + i as f32 * 66., 232., 62., 34.);
        if self.phase == Phase::Round && self.insurance_offer {
            b.push(Button {
                rect: slot(0),
                label: "INSURE",
                key: "I",
                action: Action::Insure,
                enabled: true,
                color: Color::from_rgba(58, 123, 213, 255),
            });
            b.push(Button {
                rect: slot(1),
                label: "NO",
                key: "N",
                action: Action::NoInsure,
                enabled: true,
                color: Color::from_rgba(214, 69, 69, 255),
            });
        } else if self.phase == Phase::Round {
            let turn = self.player_turn;
            let h = self.hands.get(self.active);
            let can_double = turn && h.is_some_and(|h| h.cards.len() == 2 && !h.split_aces);
            let can_split = turn
                && self.hands.len() < 4
                && h.is_some_and(|h| {
                    h.cards.len() == 2 && !h.split_aces && h.cards[0].card.points() == h.cards[1].card.points()
                });
            let defs = [
                ("HIT", "H", Action::Hit, turn, HIT_C),
                ("STAND", "S", Action::Stand, turn, STAND_C),
                ("DOUBLE", "D", Action::Double, can_double, DOUBLE_C),
                ("SPLIT", "P", Action::Split, can_split, SPLIT_C),
            ];
            for (i, (label, key, action, enabled, color)) in defs.into_iter().enumerate() {
                b.push(Button { rect: slot(i), label, key, action, enabled, color });
            }
        } else if self.phase == Phase::Shuffle {
            b.push(Button {
                rect: Rect::new(346., 232., 128., 34.),
                label: "SKIP",
                key: "SPACE",
                action: Action::Deal,
                enabled: self.t > 0.2,
                color: Color::from_rgba(90, 96, 120, 255),
            });
        } else if self.reshuffle_in.is_none() {
            b.push(Button {
                rect: Rect::new(346., 232., 128., 34.),
                label: "DEAL",
                key: "SPACE",
                action: Action::Deal,
                enabled: betting,
                color: Color::from_rgba(224, 154, 44, 255),
            });
        }
        b
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, dt: f32, input: &Input) {
        self.time += dt;
        self.t += dt;
        self.since_deal += dt;
        let target = (self.credits) as f32;
        self.shown_credits += (target - self.shown_credits) * (1. - (-dt * 7.).exp());
        if (target - self.shown_credits).abs() < 0.5 {
            self.shown_credits = target;
        }
        for f in &mut self.floaters {
            f.t += dt;
            f.pos.y -= dt * 14.;
        }
        self.floaters.retain(|f| f.t < 1.6);
        if let Some((_, t)) = &mut self.banner {
            *t -= dt;
            if *t <= 0. {
                self.banner = None;
            }
        }

        // Button clicks become actions.
        let mut actions = input.actions.clone();
        if input.click {
            for b in self.buttons() {
                if b.enabled && b.rect.contains(input.mouse) {
                    actions.push(b.action);
                }
            }
        }
        if actions.contains(&Action::Chart) {
            self.show_chart = !self.show_chart;
        }
        // Space / Enter deal between rounds and stand during the player's turn.
        if self.phase == Phase::Round && self.player_turn {
            for a in &mut actions {
                if *a == Action::Deal {
                    *a = Action::Stand;
                }
            }
        }
        if input.wheel > 0. {
            actions.push(Action::BetUp);
        } else if input.wheel < 0. {
            actions.push(Action::BetDown);
        }

        match self.phase {
            Phase::Shuffle => {
                let i = self.shuffle_step_index(self.t);
                if i != self.shuffle_step_index(self.t - dt) {
                    match self.plan.get(i).map(|s| s.0) {
                        Some(ShStep::Riffle { .. } | ShStep::LooseRiffle { .. }) => self.sfx.push(Sfx::Shuffle),
                        Some(ShStep::Moves(_) | ShStep::Roll) => self.sfx.push(Sfx::Deal),
                        Some(ShStep::Strip { .. } | ShStep::CutCards) => self.sfx.push(Sfx::Flip),
                        Some(ShStep::Push) => self.sfx.push(Sfx::Load),
                        _ => {}
                    }
                }
                if input.confirm && self.t > 0.2 {
                    self.t = self.shuffle_time();
                }
                if self.t >= self.shuffle_time() {
                    self.phase = Phase::Cut;
                    self.t = 0.;
                    self.cut_choice = SHOE_CARDS / 2;
                }
            }
            Phase::Cut => {
                if input.left {
                    self.cut_choice = self.cut_choice.saturating_sub(4).max(CUT_MIN);
                } else if input.right {
                    self.cut_choice = (self.cut_choice + 4).min(CUT_MAX);
                }
                let over = input.mouse.y > BAR.y - 30. && input.mouse.y < BAR.y + BAR.h + 50.;
                if over && (input.mouse_moved || input.click) {
                    self.cut_choice = Self::cut_from_x(input.mouse.x);
                }
                let place = actions.contains(&Action::Deal) || (input.confirm && self.t > 0.2);
                if place {
                    // A real cut: the part in front of the cut card goes to the back.
                    self.shoe.rotate_left(self.cut_choice);
                    let jitter = macroquad::rand::gen_range(-6, 7);
                    self.cut = (PENETRATION as i32 + jitter) as usize;
                    self.phase = Phase::CutAnim;
                    self.t = 0.;
                    self.sfx.push(Sfx::Flip);
                }
            }
            Phase::CutAnim => {
                let at = |k: f32, t: f32, dt: f32| t >= k && t - dt < k;
                if at(CA_INSERT, self.t, dt) || at(CA_MARK, self.t, dt) || at(CA_BACK, self.t, dt) {
                    self.sfx.push(Sfx::Flip);
                }
                if at(CA_PULL, self.t, dt) {
                    self.sfx.push(Sfx::Deal);
                }
                if at(CA_LOAD, self.t, dt) {
                    self.sfx.push(Sfx::Load);
                }
                if self.t >= CA_RETURN {
                    self.phase = Phase::Bet;
                    self.t = 0.;
                    self.burn();
                }
            }
            Phase::Bet => {
                if let Some(before) = self.reshuffle_in {
                    let r = before - dt;
                    self.reshuffle_in = Some(r);
                    if before > 0.6 && r <= 0.6 {
                        self.sweep_table();
                    }
                    if r <= 0. {
                        self.reshuffle_in = None;
                        self.begin_shuffle();
                    }
                } else {
                    for a in &actions {
                        match a {
                            Action::BetUp if self.bet_idx + 1 < BETS.len() => {
                                self.bet_idx += 1;
                                self.sfx.push(Sfx::Chip);
                            }
                            Action::BetDown if self.bet_idx > 0 => {
                                self.bet_idx -= 1;
                                self.sfx.push(Sfx::Chip);
                            }
                            _ => {}
                        }
                    }
                    if actions.contains(&Action::Deal) {
                        self.start_round();
                    }
                }
            }
            Phase::Round => {
                if self.script.is_empty() && (self.player_turn || self.insurance_offer) {
                    let enabled: Vec<Action> =
                        self.buttons().iter().filter(|b| b.enabled).map(|b| b.action).collect();
                    let playable = |a: &Action| {
                        matches!(
                            a,
                            Action::Hit | Action::Stand | Action::Double | Action::Split | Action::Insure | Action::NoInsure
                        )
                    };
                    if let Some(a) = actions.iter().find(|a| playable(a) && enabled.contains(a)) {
                        self.player_action(*a);
                    }
                }
                self.run_script(dt);
            }
        }

        // Card motion.
        let n = self.dealer.len();
        for (i, s) in self.dealer.iter_mut().enumerate() {
            s.step(Self::dealer_slot(i, n), dt);
        }
        let spans = self.hand_spans();
        for (h, (x, _)) in self.hands.iter_mut().zip(spans) {
            for (i, s) in h.cards.iter_mut().enumerate() {
                let slot = Self::hand_slot(x, i, s.sideways);
                s.step(slot, dt);
            }
        }
        let tray = DISCARD_POS - vec2(0., (self.discard / 12).min(26) as f32);
        for s in &mut self.to_discard {
            s.face_up = false;
            s.sideways = false;
            s.step(tray, dt * 0.9);
        }
        let before = self.to_discard.len();
        self.to_discard.retain(|s| s.pos.distance(tray) > 3.);
        self.discard += before - self.to_discard.len();
        let rest = if self.cut_out { CUT_CARD_REST } else { vec2(SHOE_STACK.x, SHOE_STACK.y) };
        self.cut_card_pos += (rest - self.cut_card_pos) * (1. - (-dt * 6.).exp());
    }

    /// The first card out of a fresh shoe goes face down to the discard tray.
    fn burn(&mut self) {
        let card = self.draw_card();
        self.to_discard.push(Sprite::new(card, false));
        self.since_deal = 0.;
        self.sfx.push(Sfx::Deal);
    }

    fn sweep_table(&mut self) {
        let mut moved = std::mem::take(&mut self.dealer);
        for h in self.hands.drain(..) {
            moved.extend(h.cards);
        }
        if !moved.is_empty() {
            self.sfx.push(Sfx::Deal);
        }
        self.to_discard.extend(moved);
        self.insurance = 0;
        self.insurance_paid = None;
    }

    fn start_round(&mut self) {
        self.sweep_table();
        let bet = self.bet();
        self.credits -= bet * 2;
        self.hands.push(Hand::new(bet));
        self.active = 0;
        self.player_turn = false;
        self.phase = Phase::Round;
        self.t = 0.;
        self.sfx.push(Sfx::Chip);
        self.wait = 0.15;
        self.script.extend([
            Step::Deal(Target::Player(0), true),
            Step::Deal(Target::Dealer, true),
            Step::Deal(Target::Player(0), true),
            Step::Deal(Target::Dealer, false),
            Step::AfterDeal,
        ]);
    }

    fn player_action(&mut self, a: Action) {
        self.player_turn = false;
        let h = self.active;
        match a {
            Action::Insure | Action::NoInsure => {
                self.insurance_offer = false;
                if a == Action::Insure {
                    // Half the main bet, in half credits.
                    self.insurance = self.hands[0].bet;
                    self.credits -= self.insurance;
                    self.sfx.push(Sfx::Chip);
                }
                self.wait = 0.3;
                self.script.push_back(Step::Peek);
            }
            Action::Hit => self.script.extend([Step::Deal(Target::Player(h), true), Step::Check]),
            Action::Stand => {
                self.hands[h].done = true;
                self.script.push_back(Step::NextHand);
            }
            Action::Double => {
                let bet = self.hands[h].bet;
                self.credits -= bet * 2;
                self.hands[h].bet *= 2;
                self.hands[h].doubled = true;
                self.sfx.push(Sfx::Chip);
                self.script.extend([Step::Deal(Target::Player(h), true), Step::Check]);
            }
            Action::Split => {
                let bet = self.hands[h].bet;
                self.credits -= bet * 2;
                let second = self.hands[h].cards.pop().unwrap();
                let aces = second.card.rank == 1;
                let mut nh = Hand::new(bet);
                nh.cards.push(second);
                nh.split_aces = aces;
                self.hands[h].split_aces = aces;
                self.hands.insert(h + 1, nh);
                self.sfx.push(Sfx::Chip);
                self.wait = 0.2;
                self.script.extend([Step::Deal(Target::Player(h), true), Step::Check]);
            }
            _ => {}
        }
    }

    fn draw_card(&mut self) -> Card {
        if self.next >= self.shoe.len() {
            // Only reachable with an extreme cut and many splits: open a fresh shoe.
            self.shoe = new_shoe(DECKS);
            shuffle(&mut self.shoe);
            self.next = 0;
            self.cut_out = true;
        }
        let c = self.shoe[self.next];
        self.next += 1;
        if self.next == self.cut && !self.cut_out {
            self.cut_out = true;
            self.cut_card_pos = vec2(SHOE_STACK.x - 30., SHOE_STACK.y);
            self.banner = Some(("CUT CARD! LAST HAND".into(), 1.8));
        }
        c
    }

    fn run_script(&mut self, dt: f32) {
        self.wait -= dt;
        while self.wait <= 0. {
            let Some(step) = self.script.pop_front() else {
                self.wait = 0.;
                return;
            };
            self.exec(step);
        }
    }

    fn dealer_cards(&self) -> Vec<Card> {
        self.dealer.iter().map(|s| s.card).collect()
    }

    fn exec(&mut self, step: Step) {
        match step {
            Step::Deal(target, face_up) => {
                let card = self.draw_card();
                let mut s = Sprite::new(card, face_up);
                match target {
                    Target::Dealer => self.dealer.push(s),
                    Target::Player(h) => {
                        s.sideways = self.hands[h].doubled;
                        self.hands[h].cards.push(s);
                    }
                }
                self.sfx.push(Sfx::Deal);
                self.since_deal = 0.;
                self.wait += DEAL_WAIT;
            }
            Step::AfterDeal => {
                if self.dealer[0].card.rank == 1 {
                    self.insurance_offer = true;
                } else {
                    self.script.push_back(Step::Peek);
                }
            }
            Step::Peek => {
                let up = self.dealer[0].card;
                let peek = up.rank == 1 || up.points() == 10;
                let dealer_bj = peek && is_blackjack(&self.dealer_cards());
                if self.insurance > 0 {
                    if dealer_bj {
                        // Insurance pays 2:1.
                        self.credits += self.insurance * 3;
                        self.floater_at(self.insurance * 2, INSURANCE_FLOATER_POS);
                        self.sfx.push(Sfx::Win);
                    } else {
                        self.floater_at(-self.insurance, INSURANCE_FLOATER_POS);
                    }
                    self.insurance_paid = Some(dealer_bj);
                }
                let player_bj = is_blackjack(&self.hands[0].plain());
                if dealer_bj || player_bj {
                    self.hands[0].done = true;
                    self.script.extend([Step::DealerTurn, Step::Settle]);
                } else {
                    self.active = 0;
                    self.script.push_back(Step::Check);
                }
            }
            Step::Check => {
                let h = &mut self.hands[self.active];
                let v = h.value();
                if v.total > 21 {
                    h.done = true;
                    h.outcome = Some((Outcome::Bust, -h.bet * 2));
                    let bet = h.bet;
                    self.floater(-bet * 2);
                    self.wait += 0.3;
                    self.script.push_back(Step::NextHand);
                } else if v.total == 21 || h.doubled || (h.split_aces && h.cards.len() >= 2) {
                    h.done = true;
                    self.wait += 0.15;
                    self.script.push_back(Step::NextHand);
                } else {
                    self.player_turn = true;
                }
            }
            Step::NextHand => {
                self.active += 1;
                if self.active < self.hands.len() {
                    if self.hands[self.active].cards.len() == 1 {
                        self.script.push_back(Step::Deal(Target::Player(self.active), true));
                    }
                    self.script.push_back(Step::Check);
                } else {
                    self.script.push_back(Step::DealerTurn);
                }
            }
            Step::DealerTurn => {
                self.player_turn = false;
                self.dealer[1].face_up = true;
                self.sfx.push(Sfx::Flip);
                self.wait += 0.4;
                let live = self.hands.iter().any(|h| h.outcome.is_none());
                let natural = self.hands.len() == 1 && is_blackjack(&self.hands[0].plain());
                if live && !natural && !is_blackjack(&self.dealer_cards()) {
                    self.script.push_back(Step::DealerDraw);
                } else if !matches!(self.script.front(), Some(Step::Settle)) {
                    self.script.push_back(Step::Settle);
                }
            }
            Step::DealerDraw => {
                // Dealer stands on all 17s.
                if hand_value(self.dealer_cards()).total < 17 {
                    self.script.extend([Step::Deal(Target::Dealer, true), Step::DealerDraw]);
                } else {
                    self.script.push_back(Step::Settle);
                }
            }
            Step::Settle => self.settle(),
        }
    }

    fn settle(&mut self) {
        let dealer = self.dealer_cards();
        let dv = hand_value(dealer.iter().copied()).total;
        let dealer_bj = is_blackjack(&dealer);
        let single = self.hands.len() == 1;
        let mut shown = 0;
        let mut any_win = false;
        let mut any_new = false;
        for h in &mut self.hands {
            if h.outcome.is_some() {
                continue;
            }
            any_new = true;
            let pv = h.value().total;
            let player_bj = single && is_blackjack(&h.plain());
            let stake = h.bet * 2;
            let (o, net) = if dealer_bj {
                if player_bj { (Outcome::Push, 0) } else { (Outcome::Lose, -stake) }
            } else if player_bj {
                (Outcome::Blackjack, h.bet * 3)
            } else if dv > 21 || pv > dv {
                (Outcome::Win, stake)
            } else if pv == dv {
                (Outcome::Push, 0)
            } else {
                (Outcome::Lose, -stake)
            };
            if net >= 0 {
                self.credits += stake + net;
            }
            any_win |= net > 0;
            shown += net;
            h.outcome = Some((o, net));
        }
        // Busts already got their floater when they happened.
        if any_new {
            self.floater(shown);
        }
        if any_win {
            self.sfx.push(Sfx::Win);
        }
        if self.hands.iter().any(|h| h.outcome.is_some_and(|o| o.0 == Outcome::Blackjack)) {
            self.banner = Some(("BLACKJACK!".into(), 1.4));
        }
        self.player_turn = false;
        self.phase = Phase::Bet;
        self.t = 0.;
        if self.cut_out {
            self.reshuffle_in = Some(2.2);
        }
    }

    /// The BJ_AUTO test mode's player.
    pub fn auto_action(&self) -> Option<Action> {
        if self.phase == Phase::Bet && self.reshuffle_in.is_none() && self.t > 0.5 {
            return Some(Action::Deal);
        }
        if self.phase != Phase::Round || !self.script.is_empty() {
            return None;
        }
        if self.insurance_offer {
            if self.t < 1.8 {
                return None;
            }
            return Some(if macroquad::rand::gen_range(0, 2) == 0 { Action::Insure } else { Action::NoInsure });
        }
        // Pause like a person would, so the table can be watched.
        if !self.player_turn || self.since_deal < 0.6 {
            return None;
        }
        // Plays the basic strategy chart.
        let enabled: Vec<Action> = self.buttons().iter().filter(|b| b.enabled).map(|b| b.action).collect();
        let h = &self.hands[self.active];
        let cards: Vec<Card> = h.cards.iter().map(|s| s.card).collect();
        let can_split = enabled.contains(&Action::Split);
        let can_double = enabled.contains(&Action::Double);
        let Some((t, r, c)) = crate::strategy::lookup(&cards, self.dealer[0].card, can_split) else {
            return Some(Action::Stand);
        };
        Some(match crate::strategy::play(t, r, c) {
            'P' => Action::Split,
            'D' if can_double => Action::Double,
            'd' if can_double => Action::Double,
            'D' | 'H' => Action::Hit,
            _ => Action::Stand,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Input {
        Input {
            mouse: Vec2::ZERO,
            mouse_moved: false,
            click: false,
            wheel: 0.,
            actions: Vec::new(),
            confirm: false,
            left: false,
            right: false,
        }
    }

    fn act(a: Action) -> Input {
        Input { actions: vec![a], confirm: a == Action::Deal, ..idle() }
    }

    fn table_cards(g: &Game) -> usize {
        g.remaining() + g.discard + g.to_discard.len() + g.dealer.len() + g.hands.iter().map(|h| h.cards.len()).sum::<usize>()
    }

    /// Runs the clock until the game waits for the player (or the round is over).
    fn settle_time(g: &mut Game) {
        for _ in 0..400 {
            g.update(0.05, &idle());
            let waiting = g.script.is_empty() && (g.player_turn || g.insurance_offer);
            if (g.phase == Phase::Round && waiting) || g.phase == Phase::Bet {
                break;
            }
        }
    }

    /// A game at the betting stage whose shoe starts with `top` (rank only, suits cycle).
    fn stacked(top: &[u8]) -> Game {
        let mut g = Game::new();
        g.update(0.3, &idle());
        g.update(0.05, &act(Action::Deal)); // skip the shuffle
        g.update(0.05, &idle());
        g.update(0.3, &idle());
        g.update(0.05, &act(Action::Deal)); // cut
        for _ in 0..100 {
            g.update(0.05, &idle());
        }
        assert_eq!(g.phase, Phase::Bet);
        // The burn card is already gone.
        assert_eq!(g.next, 1);
        for (i, r) in top.iter().enumerate() {
            g.shoe[g.next + i] = Card { rank: *r, suit: (i % 4) as u8 };
        }
        g.cut = CUT_MAX;
        g
    }

    fn start(g: &mut Game) -> i64 {
        let before = g.credits;
        g.update(0.05, &act(Action::Deal));
        settle_time(g);
        before
    }

    #[test]
    fn shuffle_plan_keeps_every_card() {
        let mut g = Game::new();
        let total = g.shuffle_time();
        let mut t = 0.;
        while t < total + 0.1 {
            g.t = t;
            // Counts are taken at the start of each step, so after the gather
            // every card is always in some pile.
            if t > 0.6 {
                assert_eq!(g.shuffle_view().counts.iter().sum::<usize>(), SHOE_CARDS, "at t={t}");
            }
            t += 0.01;
        }
        let end = g.shuffle_view();
        assert_eq!(end.counts[M], SHOE_CARDS);
        assert!(end.rolled && end.cut_cards && end.step.is_none());
        let count = |f: fn(&ShStep) -> bool| g.plan.iter().filter(|s| f(&s.0)).count();
        // Five working-stack grabs of riffle, riffle, strip, riffle; six loose final riffles.
        assert_eq!(count(|s| matches!(s, ShStep::Riffle { .. })), 15);
        assert_eq!(count(|s| matches!(s, ShStep::Strip { .. })), 5);
        assert_eq!(count(|s| matches!(s, ShStep::LooseRiffle { .. })), 6);
    }

    #[test]
    fn opening_the_chart_keeps_the_turn() {
        let mut g = stacked(&[10, 10, 6, 7, 2]);
        start(&mut g);
        assert!(g.player_turn);
        g.update(0.05, &act(Action::Chart));
        assert!(g.show_chart);
        assert!(g.player_turn);
        g.update(0.05, &act(Action::Hit));
        settle_time(&mut g);
        assert_eq!(g.hands[0].cards.len(), 3);
    }

    #[test]
    fn space_stands_during_the_turn() {
        let mut g = stacked(&[10, 10, 7, 7]);
        start(&mut g);
        assert!(g.player_turn);
        g.update(0.05, &act(Action::Deal));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.hands[0].cards.len(), 2);
    }

    #[test]
    fn cut_swaps_the_two_parts_and_marks_a_deck_from_the_end() {
        let mut g = Game::new();
        g.update(0.3, &idle());
        g.update(0.05, &act(Action::Deal));
        g.update(0.05, &idle());
        assert_eq!(g.phase, Phase::Cut);
        let before = g.shoe.clone();
        g.cut_choice = 100;
        g.update(0.3, &idle());
        g.update(0.05, &act(Action::Deal));
        assert_eq!(g.phase, Phase::CutAnim);
        assert_eq!(g.shoe[0], before[100]);
        assert_eq!(g.shoe[SHOE_CARDS - 100], before[0]);
        assert!((PENETRATION - 6..=PENETRATION + 6).contains(&g.cut));
        for _ in 0..70 {
            g.update(0.05, &idle());
        }
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.shoe_body(), SHOE_BODY);
        // One card burned, lying in (or flying to) the discard tray.
        assert_eq!(g.next, 1);
        assert_eq!(g.discard + g.to_discard.len(), 1);
    }

    #[test]
    fn blackjack_pays_3_to_2() {
        // P A, D 9, P K, D 7
        let mut g = stacked(&[1, 9, 13, 7]);
        let before = start(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.credits - before, 15 * 2);
        assert_eq!(g.hands[0].outcome.unwrap().0, Outcome::Blackjack);
    }

    #[test]
    fn dealer_blackjack_takes_the_bet() {
        let mut g = stacked(&[10, 1, 10, 12]);
        let before = start(&mut g);
        assert!(g.insurance_offer);
        g.update(0.05, &act(Action::NoInsure));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.credits - before, -20);
        assert_eq!(g.hands[0].outcome.unwrap().0, Outcome::Lose);
    }

    #[test]
    fn insurance_pays_2_to_1() {
        // Bet 10, insure 5. Dealer blackjack: lose 10, insurance wins 10.
        let mut g = stacked(&[10, 1, 10, 12]);
        let before = start(&mut g);
        g.update(0.05, &act(Action::Insure));
        assert_eq!(before - g.credits, 20 + 10);
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.insurance_paid, Some(true));
        assert_eq!(g.credits, before);
    }

    #[test]
    fn insurance_lost_when_no_blackjack() {
        // Dealer A+9 = 20, player 19: lose 10 and the 5 insurance.
        let mut g = stacked(&[10, 1, 9, 9]);
        let before = start(&mut g);
        g.update(0.05, &act(Action::Insure));
        settle_time(&mut g);
        assert_eq!(g.insurance_paid, Some(false));
        assert!(g.player_turn);
        g.update(0.05, &act(Action::Stand));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.credits - before, -30);
    }

    #[test]
    fn no_insurance_offer_without_ace() {
        let mut g = stacked(&[10, 10, 7, 7]);
        start(&mut g);
        assert!(!g.insurance_offer);
        assert!(g.player_turn);
    }

    #[test]
    fn both_blackjack_push() {
        let mut g = stacked(&[1, 1, 10, 10]);
        let before = start(&mut g);
        g.update(0.05, &act(Action::NoInsure));
        settle_time(&mut g);
        assert_eq!(g.credits, before);
        assert_eq!(g.hands[0].outcome.unwrap().0, Outcome::Push);
    }

    #[test]
    fn double_down_win() {
        // P 6, D 10, P 5, D 7, P (double) 10 -> 21 vs 17
        let mut g = stacked(&[6, 10, 5, 7, 10]);
        let before = start(&mut g);
        assert!(g.player_turn);
        g.update(0.05, &act(Action::Double));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.hands[0].cards.len(), 3);
        assert!(g.hands[0].cards[2].sideways);
        assert_eq!(g.credits - before, 20 * 2);
    }

    #[test]
    fn split_eights() {
        // P 8, D 10, P 8, D 6, then hand 1 gets 3 (11) -> stand, hand 2 gets 10 (18) -> stand,
        // dealer 16 draws 9 -> 25 bust. Both hands win.
        let mut g = stacked(&[8, 10, 8, 6, 3, 10, 9]);
        let before = start(&mut g);
        g.update(0.05, &act(Action::Split));
        settle_time(&mut g);
        assert_eq!(g.hands.len(), 2);
        assert_eq!(g.active, 0);
        g.update(0.05, &act(Action::Stand));
        settle_time(&mut g);
        assert_eq!(g.active, 1);
        g.update(0.05, &act(Action::Stand));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(hand_value(g.dealer.iter().map(|s| s.card)).total, 25);
        assert_eq!(g.credits - before, 20 * 2);
    }

    #[test]
    fn split_aces_get_one_card_each() {
        let mut g = stacked(&[1, 10, 1, 7, 13, 5]);
        let before = start(&mut g);
        g.update(0.05, &act(Action::Split));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.hands[0].cards.len(), 2);
        assert_eq!(g.hands[1].cards.len(), 2);
        // A+K is 21 (not a blackjack after a split) vs 17: win 10; A+5 = 16 vs 17: lose 10.
        assert_eq!(g.hands[0].outcome.unwrap().0, Outcome::Win);
        assert_eq!(g.hands[1].outcome.unwrap().0, Outcome::Lose);
        assert_eq!(g.credits, before);
    }

    #[test]
    fn bust_loses_and_dealer_skips_drawing() {
        let mut g = stacked(&[10, 6, 6, 10, 10]);
        let before = start(&mut g);
        g.update(0.05, &act(Action::Hit));
        settle_time(&mut g);
        assert_eq!(g.phase, Phase::Bet);
        assert_eq!(g.hands[0].outcome.unwrap().0, Outcome::Bust);
        assert_eq!(g.dealer.len(), 2);
        assert_eq!(g.credits - before, -20);
    }

    /// Long self-played session: the shoe is reshuffled when the cut card comes out,
    /// no card is ever lost, and credits are allowed to go anywhere.
    #[test]
    fn endless_session() {
        let mut g = Game::new();
        let mut shuffles = 0;
        let mut rounds = 0;
        let mut was_bet = false;
        let mut max_hands = 0;
        let mut insured = 0;
        for _ in 0..400_000 {
            let mut input = idle();
            input.actions.extend(g.auto_action());
            input.confirm = g.t > 0.6;
            let prev = g.phase;
            g.update(0.05, &input);
            if prev != Phase::Shuffle && g.phase == Phase::Shuffle {
                shuffles += 1;
            }
            let is_bet = g.phase == Phase::Bet;
            if is_bet && !was_bet && prev == Phase::Round {
                rounds += 1;
                assert!(g.hands.iter().all(|h| h.outcome.is_some()));
                insured += (g.insurance > 0) as usize;
            }
            was_bet = is_bet;
            max_hands = max_hands.max(g.hands.len());
            assert_eq!(table_cards(&g), SHOE_CARDS, "cards lost in {:?}", g.phase);
        }
        assert!(rounds > 1000, "rounds {rounds}");
        assert!(shuffles > 15, "shuffles {shuffles}");
        assert!(max_hands >= 2);
        assert!(insured > 0);
    }
}
