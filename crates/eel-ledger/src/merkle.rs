//! Deterministic binary Merkle tree over event ids.
//!
//! * leaf  = BLAKE3(0x00 || event_id)
//! * node  = BLAKE3(0x01 || left || right)
//! * an odd node at the end of a level is promoted unchanged (never
//!   duplicated, which avoids the duplicate-leaf ambiguity of some designs);
//! * the root of an empty list is 32 zero bytes.

use eel_core::{EventId, Hash};

pub fn leaf(id: &EventId) -> Hash {
    let mut buf = Vec::with_capacity(33);
    buf.push(0x00);
    buf.extend_from_slice(id.as_hash().as_bytes());
    Hash::digest(&buf)
}

fn node(l: &Hash, r: &Hash) -> Hash {
    let mut buf = Vec::with_capacity(65);
    buf.push(0x01);
    buf.extend_from_slice(l.as_bytes());
    buf.extend_from_slice(r.as_bytes());
    Hash::digest(&buf)
}

pub fn root(ids: &[EventId]) -> Hash {
    if ids.is_empty() {
        return Hash::ZERO;
    }
    let mut level: Vec<Hash> = ids.iter().map(leaf).collect();
    while level.len() > 1 {
        level = level
            .chunks(2)
            .map(|c| {
                if c.len() == 2 {
                    node(&c[0], &c[1])
                } else {
                    c[0]
                }
            })
            .collect();
    }
    level[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: u8) -> Vec<EventId> {
        (0..n).map(|i| EventId(Hash::digest(&[i]))).collect()
    }

    #[test]
    fn root_is_deterministic_and_order_sensitive() {
        assert_eq!(root(&ids(5)), root(&ids(5)));
        let mut rev = ids(5);
        rev.reverse();
        assert_ne!(root(&ids(5)), root(&rev));
        assert_ne!(root(&ids(4)), root(&ids(5)));
        assert_eq!(root(&[]), Hash::ZERO);
    }

    #[test]
    fn odd_leaf_is_not_duplicated() {
        let three = ids(3);
        let mut four = three.clone();
        four.push(three[2]);
        assert_ne!(root(&three), root(&four));
    }
}
