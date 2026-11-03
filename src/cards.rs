use macroquad::rand;

/// rank: 1 = Ace .. 13 = King. suit: 0 hearts, 1 diamonds, 2 clubs, 3 spades
/// (same row order as the Kenney tilesheet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    pub rank: u8,
    pub suit: u8,
}

impl Card {
    pub fn points(self) -> u32 {
        match self.rank {
            11..=13 => 10,
            r => r as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Value {
    pub total: u32,
    pub soft: bool,
}

pub fn hand_value(cards: impl IntoIterator<Item = Card>) -> Value {
    let mut total = 0;
    let mut ace = false;
    for c in cards {
        total += c.points();
        ace |= c.rank == 1;
    }
    if ace && total + 10 <= 21 {
        Value { total: total + 10, soft: true }
    } else {
        Value { total, soft: false }
    }
}

pub fn is_blackjack(cards: &[Card]) -> bool {
    cards.len() == 2 && hand_value(cards.iter().copied()).total == 21
}

pub fn new_shoe(decks: usize) -> Vec<Card> {
    let mut v = Vec::with_capacity(decks * 52);
    for _ in 0..decks {
        for suit in 0..4 {
            for rank in 1..=13 {
                v.push(Card { rank, suit });
            }
        }
    }
    v
}

/// Fisher-Yates.
pub fn shuffle(v: &mut [Card]) {
    for i in (1..v.len()).rev() {
        let j = rand::gen_range(0, i + 1);
        v.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8) -> Card {
        Card { rank, suit: 0 }
    }

    #[test]
    fn values() {
        assert_eq!(hand_value([c(1), c(13)]), Value { total: 21, soft: true });
        assert_eq!(hand_value([c(1), c(6)]), Value { total: 17, soft: true });
        assert_eq!(hand_value([c(1), c(6), c(10)]), Value { total: 17, soft: false });
        assert_eq!(hand_value([c(1), c(1), c(9)]), Value { total: 21, soft: true });
        assert_eq!(hand_value([c(10), c(12), c(5)]), Value { total: 25, soft: false });
        assert_eq!(hand_value([c(1), c(1)]), Value { total: 12, soft: true });
    }

    #[test]
    fn blackjack() {
        assert!(is_blackjack(&[c(1), c(11)]));
        assert!(!is_blackjack(&[c(5), c(6), c(10)]));
        assert!(!is_blackjack(&[c(10), c(9)]));
    }

    #[test]
    fn shoe_is_complete() {
        let mut s = new_shoe(6);
        shuffle(&mut s);
        assert_eq!(s.len(), 312);
        for suit in 0..4 {
            for rank in 1..=13 {
                assert_eq!(s.iter().filter(|x| **x == Card { rank, suit }).count(), 6);
            }
        }
    }
}
