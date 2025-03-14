/*use std::sync::Arc;

//

/// raw byte data rope
pub enum Rope {
    Branch { next: Arc<[Rope; 2]>, length: usize },
    Leaf([u8; 16]),
}

impl Rope {
    pub const fn new() -> Self {
        Self::Leaf([const { 0 }; 16])
    }

    pub const fn new_short(b: &[u8]) -> Option<Self> {
        if b.len() >= 16 {
            None
        } else {
            let mut whole = [const { 0u8 }; 16];
            whole[0] = b.len() as u8;
        }

        Self::Leaf([const { 0 }; 16])
    }
}

impl Default for Rope {
    fn default() -> Self {
        Self::new()
    }
}*/
