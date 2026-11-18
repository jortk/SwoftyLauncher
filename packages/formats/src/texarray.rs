// Texture array layout and slice merge ported from src/texarray.rs of the Team
// Simplified addon for Swofty Launcher, which ports them from merge_resources.py of
// the Simplified Launcher and LogoBaker.cs and LogoSource.cs of the Simplified Team Editor.
// Based on Simplified Suite by Team Simplified - https://github.com/TeamSimplified/simplified-suite
// Copyright (C) 2026 Team Simplified. GPL-3.0; see THIRD_PARTY.md.

use crate::erp::Resource;

// Fragment 0 of a surface holds its header: the image type at 8, the size at 12 and
// 16, the mip count at 24 and the array size at 28. Fragment 1 holds every slice.
const LAYOUT: std::ops::Range<usize> = 8..28;
const COUNT: usize = 28;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Array {
    pub layout: Vec<u8>,
    pub count: usize,
    pub data: Vec<u8>,
}

impl Array {
    pub fn read(res: &Resource) -> Option<Array> {
        let (f0, f1) = (res.fragments.first()?, res.fragments.get(1)?);
        let head = f0.data().ok()?;
        let count = u32::from_le_bytes(head.get(COUNT..COUNT + 4)?.try_into().ok()?) as usize;
        let data = f1.data().ok()?;
        if count < 2 || data.is_empty() || data.len() % count != 0 {
            return None;
        }
        Some(Array {
            layout: head[LAYOUT].to_vec(),
            count,
            data,
        })
    }

    pub fn slice_len(&self) -> usize {
        self.data.len() / self.count
    }

    pub fn slice(&self, i: usize) -> &[u8] {
        let n = self.slice_len();
        &self.data[i * n..(i + 1) * n]
    }

    // Arrays built for an older game can hold fewer slices than the game's own; the
    // slices they hold sit at the same places.
    pub fn fits(&self, stock: &Array) -> bool {
        self.layout == stock.layout && self.slice_len() == stock.slice_len()
    }

    pub fn same_layout(&self, stock: &Array) -> bool {
        self.fits(stock) && self.count == stock.count
    }

    /// An array of one slice or more: a source for `append`.
    pub fn read_slices(res: &Resource) -> Option<Array> {
        let (f0, f1) = (res.fragments.first()?, res.fragments.get(1)?);
        let head = f0.data().ok()?;
        let count = u32::from_le_bytes(head.get(COUNT..COUNT + 4)?.try_into().ok()?) as usize;
        let data = f1.data().ok()?;
        if count == 0 || data.is_empty() || data.len() % count != 0 {
            return None;
        }
        Some(Array {
            layout: head[LAYOUT].to_vec(),
            count,
            data,
        })
    }

    /// Puts every slice of `source` after this array's slices and returns their
    /// indices. `source` fits this array.
    pub fn append(&mut self, source: &Array) -> std::ops::Range<usize> {
        let at = self.count;
        self.data.extend_from_slice(&source.data);
        self.count += source.count;
        at..self.count
    }

    /// Fragment 0 of a surface with its array size set to `count`.
    pub fn with_count(head: &[u8], count: usize) -> Option<Vec<u8>> {
        let mut head = head.to_vec();
        head.get_mut(COUNT..COUNT + 4)?
            .copy_from_slice(&(count as u32).to_le_bytes());
        Some(head)
    }

    /// Writes each slice of `source` that differs from `stock` over this array and
    /// returns their indices. This array and `source` fit `stock`; the slice count of
    /// this array stays, so a slice past it is left out.
    pub fn merge(&mut self, stock: &Array, source: &Array) -> Vec<usize> {
        let n = stock.slice_len();
        let mut changed = Vec::new();
        for i in 0..source.count.min(stock.count).min(self.count) {
            let theirs = source.slice(i);
            if theirs != stock.slice(i) {
                self.data[i * n..(i + 1) * n].copy_from_slice(theirs);
                changed.push(i);
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::erp::Fragment;

    fn fragment(data: Vec<u8>) -> Fragment {
        Fragment {
            name: *b"temp",
            offset: 0,
            size: data.len() as u64,
            flags: 0x10,
            compression: crate::erp::STORED,
            packed: data.len() as u64,
            raw: data,
        }
    }

    fn surface(width: u32, slices: &[[u8; 4]]) -> Resource {
        let mut head = vec![0u8; 40];
        head[12..16].copy_from_slice(&width.to_le_bytes());
        head[28..32].copy_from_slice(&(slices.len() as u32).to_le_bytes());
        Resource {
            name: "eaid://ui_package/a.texarray.image".into(),
            kind: [0; 16],
            unk: vec![0; 4],
            fragments: vec![fragment(head), fragment(slices.concat())],
            hash: [0; 16],
        }
    }

    #[test]
    fn reads_slices_and_merges_the_ones_that_differ_from_stock() {
        let stock = Array::read(&surface(2, &[[0; 4], [1; 4], [2; 4], [3; 4]])).unwrap();
        assert_eq!(
            (stock.count, stock.slice_len(), stock.slice(2)),
            (4, 4, &[2u8; 4][..])
        );
        let mut base = Array::read(&surface(2, &[[0; 4], [9; 4], [2; 4], [3; 4]])).unwrap();
        let older = Array::read(&surface(2, &[[6; 4], [1; 4]])).unwrap();
        assert!(older.fits(&stock) && !older.same_layout(&stock));
        assert_eq!(base.merge(&stock, &older), [0]);
        assert_eq!(base.data, [[6; 4], [9; 4], [2; 4], [3; 4]].concat());
        let longer = Array::read(&surface(2, &[[0; 4], [1; 4], [2; 4], [3; 4], [7; 4]])).unwrap();
        assert!(base.merge(&stock, &longer).is_empty());
        let mut grown = Array::read(&surface(2, &[[0; 4], [1; 4], [2; 4], [3; 4], [8; 4], [9; 4]])).unwrap();
        let one = Array::read(&surface(2, &[[0; 4], [1; 4], [5; 4], [3; 4]])).unwrap();
        assert_eq!(grown.merge(&stock, &one), [2]);
        assert_eq!(
            grown.data,
            [[0; 4], [1; 4], [5; 4], [3; 4], [8; 4], [9; 4]].concat()
        );
        let mut short = Array::read(&surface(2, &[[0; 4], [1; 4]])).unwrap();
        assert_eq!(short.merge(&stock, &one), Vec::<usize>::new());
        assert_eq!((short.count, short.data.len()), (2, 8));
        let wide = Array::read(&surface(4, &[[0; 4], [1; 4], [2; 4], [3; 4]])).unwrap();
        assert!(!wide.fits(&stock));
        assert!(Array::read(&surface(2, &[[0; 4]])).is_none());
        let one = Array::read_slices(&surface(2, &[[5; 4]])).unwrap();
        assert!(Array::read(&surface(2, &[[5; 4]])).is_none() && one.fits(&stock));
        let mut grown = stock.clone();
        assert_eq!(grown.append(&one), 4..5);
        assert_eq!((grown.count, grown.slice(4)), (5, &[5u8; 4][..]));
        let head = Array::with_count(&[0u8; 40], 5).unwrap();
        assert_eq!(&head[28..32], &5u32.to_le_bytes());
        let mut empty = surface(2, &[[0; 4], [1; 4]]);
        empty.fragments[1] = fragment(Vec::new());
        assert!(Array::read(&empty).is_none());
    }
}
