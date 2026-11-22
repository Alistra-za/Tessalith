//! The endpoint pairs `encode` has already scored in one block, so none is scored twice (Note HY8).
//!
//! Why skipping a repeat cannot change a byte, at every level (Note HY15): in `total_orderings`, in the 3-colour
//! ordering loop and in `endpoint_search` the error a trial must beat only falls, and when each starts it is at most
//! the error of every pair scored before it in the block for that block type. A pair scored before was either taken,
//! so its error is at least the present best, or rejected against an error that was at least the present best, so it
//! can never be strictly lower now, and a trial is taken only when strictly lower. The argument needs nothing of the
//! level's `Params`: with one round `cur_err` is that round's final error, at most every error it scored; with one
//! ordering pass the pass starts from that error; the 3-colour set is `try_3color_block`'s own; the search starts from
//! the block's best, whatever its number of rounds. The least-squares passes of a second round are different:
//! `round_err` starts fresh in each round, so a pair round 0 scored can be taken there (they record but never skip).

use super::params::{most, most3, params, LEVELS};
use super::Ends;

/// The endpoint pairs already scored in one block, for one block type (4-colour or 3-colour).
pub(crate) struct Seen {
    /// 0 is an empty slot, anything else is the pair's 32-bit key plus 1.
    slots: [u64; Seen::SLOTS],
}

impl Seen {
    pub(crate) const SLOTS: usize = 1024;
    /// The most pairs one block can score for one block type, over every level: the most of [`most`] (518 at level
    /// 18: 2 round starts, 4 least-squares trials, 2 x 128 ordering trials and 256 search steps).
    pub(crate) const MOST: usize = {
        let mut m = 0;
        let mut i = 0;
        while i < LEVELS.len() {
            let n = most(params(LEVELS[i]));
            if n > m {
                m = n;
            }
            i += 1;
        }
        m
    };

    #[inline(always)]
    pub(crate) fn new() -> Self {
        Seen {
            slots: [0; Seen::SLOTS],
        }
    }

    /// Records `e`. True when it was recorded before.
    #[inline(always)]
    pub(crate) fn insert(&mut self, e: &Ends) -> bool {
        self.insert_key(Self::key(e))
    }

    /// [`Seen::insert`] for the pair whose key is `key`.
    #[inline(always)]
    pub(crate) fn insert_key(&mut self, key: u32) -> bool {
        let stored = key as u64 + 1;
        let mut i = Self::first_slot_of(stored);
        // At most `MOST` of the `SLOTS` slots are ever full (the const assert below), so the walk reaches an empty slot.
        loop {
            let s = self.slots[i];
            if s == stored {
                return true;
            }
            if s == 0 {
                self.slots[i] = stored;
                return false;
            }
            i = (i + 1) & (Seen::SLOTS - 1);
        }
    }

    /// The pair as 32 bits, one key per pair: 5 + 6 + 5 bits for each endpoint (`l` low, `h` high).
    #[inline(always)]
    pub(crate) fn key(e: &Ends) -> u32 {
        debug_assert!(e.l[0] < 32 && e.l[1] < 64 && e.l[2] < 32);
        debug_assert!(e.h[0] < 32 && e.h[1] < 64 && e.h[2] < 32);
        debug_assert!(e.l.iter().chain(e.h.iter()).all(|&v| v >= 0));
        (e.l[0] | e.l[1] << 5 | e.l[2] << 11 | e.h[0] << 16 | e.h[1] << 21 | e.h[2] << 27) as u32
    }

    /// The pair whose key is `key`: the inverse of [`Seen::key`].
    #[inline(always)]
    pub(crate) fn ends_of(key: u32) -> Ends {
        Ends {
            l: [
                (key & 31) as i32,
                ((key >> 5) & 63) as i32,
                ((key >> 11) & 31) as i32,
            ],
            h: [
                ((key >> 16) & 31) as i32,
                ((key >> 21) & 63) as i32,
                ((key >> 27) & 31) as i32,
            ],
        }
    }

    /// A multiplicative hash of the stored value onto 10 bits.
    #[inline(always)]
    fn first_slot_of(stored: u64) -> usize {
        (stored.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 54) as usize
    }

    #[cfg(test)]
    fn first_slot(e: &Ends) -> usize {
        Self::first_slot_of(Self::key(e) as u64 + 1)
    }
}

// The table is about half full at worst, so a probe always reaches an empty slot. The hash takes 10 bits. Both block
// types' sets stay within `MOST` at every level.
const _: () = assert!(Seen::MOST < Seen::SLOTS);
const _: () = {
    let mut i = 0;
    while i < LEVELS.len() {
        let p = params(LEVELS[i]);
        assert!(most(p) <= Seen::MOST && most3(p) <= Seen::MOST);
        i += 1;
    }
};
const _: () = assert!(Seen::SLOTS == 1 << 10);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pair_is_new_once() {
        let mut s = Seen::new();
        let e = Ends {
            l: [3, 40, 5],
            h: [20, 9, 31],
        };
        assert!(!s.insert(&e));
        assert!(s.insert(&e));
        assert!(
            !s.insert(&Ends { l: e.h, h: e.l }),
            "the swapped pair is another pair"
        );
    }

    #[test]
    fn the_all_zero_and_the_all_ones_pair_are_pairs_too() {
        let mut s = Seen::new();
        let zero = Ends {
            l: [0; 3],
            h: [0; 3],
        };
        let ones = Ends {
            l: [31, 63, 31],
            h: [31, 63, 31],
        };
        assert!(!s.insert(&zero) && !s.insert(&ones));
        assert!(s.insert(&zero) && s.insert(&ones));
    }

    #[test]
    fn holds_the_most_a_block_can_score() {
        // 518 pairs that differ only in the high bits, so the hash sends many of them to nearby slots.
        let pairs: Vec<Ends> = (0..Seen::MOST as i32)
            .map(|i| Ends {
                l: [0, 0, 0],
                h: [i & 31, (i >> 5) & 63, 7],
            })
            .collect();
        let mut s = Seen::new();
        assert!(pairs.iter().all(|e| !s.insert(e)));
        assert!(pairs.iter().all(|e| s.insert(e)));
    }

    #[test]
    fn a_key_and_its_pair_round_trip() {
        // Every low half with a few high halves, and the reverse.
        for half in 0..=0xFFFFu32 {
            for other in [0u32, 0xFFFF, 0x5A3C, 0x8001] {
                for k in [half | other << 16, other | half << 16] {
                    assert_eq!(Seen::key(&Seen::ends_of(k)), k);
                }
            }
        }
    }

    #[test]
    fn insert_key_is_insert() {
        let e = Ends {
            l: [3, 40, 5],
            h: [20, 9, 31],
        };
        let mut s = Seen::new();
        assert!(!s.insert_key(Seen::key(&e)));
        assert!(s.insert(&e) && s.insert_key(Seen::key(&e)));
    }

    /// Two different pairs whose hash names the same first slot.
    fn two_pairs_with_one_first_slot() -> (Ends, Ends) {
        let mut first = [None; Seen::SLOTS];
        for k in 0..u32::MAX {
            let e = Seen::ends_of(k);
            let slot = Seen::first_slot(&e);
            match first[slot] {
                Some(other) => return (Seen::ends_of(other), e),
                None => first[slot] = Some(k),
            }
        }
        unreachable!("1,024 slots cannot hold 2^32 keys apart")
    }

    #[test]
    fn pairs_that_share_a_slot_stay_apart() {
        let (a, b) = two_pairs_with_one_first_slot();
        assert_ne!(a, b);
        assert_eq!(Seen::first_slot(&a), Seen::first_slot(&b));
        let mut s = Seen::new();
        assert!(!s.insert(&a));
        assert!(!s.insert(&b));
        assert!(s.insert(&a) && s.insert(&b));
    }

    #[test]
    fn a_walk_past_the_last_slot_goes_on_at_the_first() {
        let last = Seen::SLOTS - 1;
        let mut in_last = (0..u32::MAX)
            .map(Seen::ends_of)
            .filter(|e| Seen::first_slot(e) == last);
        let (a, b) = (in_last.next().unwrap(), in_last.next().unwrap());
        let mut s = Seen::new();
        assert!(!s.insert(&a) && !s.insert(&b));
        assert!(
            s.slots[last] != 0 && s.slots[0] != 0,
            "the second pair sits in slot 0"
        );
        assert!(s.insert(&a) && s.insert(&b));
    }
}
