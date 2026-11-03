//! Basic strategy for these rules: 6 decks, dealer stands on all 17s, double
//! on any two cards, double after split, no surrender.
//!
//! Each row lists the play against dealer 2, 3, 4, 5, 6, 7, 8, 9, 10, A.
//! H hit, S stand, D double (else hit), d double (else stand), P split.

use crate::cards::{hand_value, Card};

pub const HARD: [(&str, &str); 10] = [
    ("8", "HHHHHHHHHH"),
    ("9", "HDDDDHHHHH"),
    ("10", "DDDDDDDDHH"),
    ("11", "DDDDDDDDDH"),
    ("12", "HHSSSHHHHH"),
    ("13", "SSSSSHHHHH"),
    ("14", "SSSSSHHHHH"),
    ("15", "SSSSSHHHHH"),
    ("16", "SSSSSHHHHH"),
    ("17", "SSSSSSSSSS"),
];

pub const SOFT: [(&str, &str); 8] = [
    ("A2", "HHHDDHHHHH"),
    ("A3", "HHHDDHHHHH"),
    ("A4", "HHDDDHHHHH"),
    ("A5", "HHDDDHHHHH"),
    ("A6", "HDDDDHHHHH"),
    ("A7", "SddddSSHHH"),
    ("A8", "SSSSSSSSSS"),
    ("A9", "SSSSSSSSSS"),
];

pub const PAIRS: [(&str, &str); 10] = [
    ("22", "PPPPPPHHHH"),
    ("33", "PPPPPPHHHH"),
    ("44", "HHHPPHHHHH"),
    ("55", "DDDDDDDDHH"),
    ("66", "PPPPPHHHHH"),
    ("77", "PPPPPPHHHH"),
    ("88", "PPPPPPPPPP"),
    ("99", "PPPPPSPPSS"),
    ("TT", "SSSSSSSSSS"),
    ("AA", "PPPPPPPPPP"),
];

pub const DEALER: [&str; 10] = ["2", "3", "4", "5", "6", "7", "8", "9", "T", "A"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Table {
    Hard,
    Soft,
    Pairs,
}

/// Which chart cell applies: (table, row, dealer column). None when there is
/// nothing to decide (21 or more, or a soft 12 that cannot be split).
pub fn lookup(hand: &[Card], up: Card, can_split: bool) -> Option<(Table, usize, usize)> {
    let col = match up.points() {
        1 => 9,
        p => p as usize - 2,
    };
    if can_split && hand.len() == 2 && hand[0].points() == hand[1].points() {
        let row = match hand[0].points() {
            1 => 9,
            p => p as usize - 2,
        };
        return Some((Table::Pairs, row, col));
    }
    let v = hand_value(hand.iter().copied());
    if v.total >= 21 {
        return None;
    }
    if v.soft {
        // Soft 13 (A2) .. soft 20 (A9).
        return (13..=20).contains(&v.total).then(|| (Table::Soft, v.total as usize - 13, col));
    }
    let row = (v.total.clamp(8, 17) - 8) as usize;
    Some((Table::Hard, row, col))
}

pub fn play(t: Table, row: usize, col: usize) -> char {
    let rows: &[(&str, &str)] = match t {
        Table::Hard => &HARD,
        Table::Soft => &SOFT,
        Table::Pairs => &PAIRS,
    };
    rows[row].1.as_bytes()[col] as char
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8) -> Card {
        Card { rank, suit: 0 }
    }

    fn advice(hand: &[u8], up: u8) -> char {
        let h: Vec<Card> = hand.iter().map(|r| c(*r)).collect();
        let (t, r, col) = lookup(&h, c(up), true).unwrap();
        play(t, r, col)
    }

    #[test]
    fn known_plays() {
        assert_eq!(advice(&[10, 6], 10), 'H'); // hard 16 vs 10
        assert_eq!(advice(&[10, 6], 6), 'S');
        assert_eq!(advice(&[10, 2], 3), 'H'); // 12 vs 3
        assert_eq!(advice(&[6, 5], 1), 'H'); // 11 vs A (S17)
        assert_eq!(advice(&[6, 5], 10), 'D');
        assert_eq!(advice(&[1, 7], 3), 'd'); // A7 vs 3
        assert_eq!(advice(&[1, 7], 9), 'H');
        assert_eq!(advice(&[8, 8], 1), 'P');
        assert_eq!(advice(&[9, 9], 7), 'S');
        assert_eq!(advice(&[5, 5], 9), 'D'); // 5s play as hard 10
        assert_eq!(advice(&[13, 12], 6), 'S'); // tens never split
        assert_eq!(advice(&[2, 3, 2], 5), 'H'); // hard 7 uses the 8 row
        assert_eq!(advice(&[10, 9], 1), 'S'); // 19 uses the 17+ row
        assert_eq!(advice(&[1, 2, 3], 5), 'D'); // soft 16 reads as A5
    }

    #[test]
    fn no_advice_on_21() {
        assert!(lookup(&[c(1), c(10)], c(5), true).is_none());
    }
}
