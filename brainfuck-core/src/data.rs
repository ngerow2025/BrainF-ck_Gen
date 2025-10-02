use std::fmt::Display;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BfInstruction {
    Inc = 0,
    Dec,
    Left,
    Right,
    LoopStart,
    LoopEnd,
    Input,
    Output,
}

impl BfInstruction {
    pub(crate) fn from_u8(n: u8) -> Option<BfInstruction> {
        match n {
            0 => Some(BfInstruction::Inc),
            1 => Some(BfInstruction::Dec),
            2 => Some(BfInstruction::Left),
            3 => Some(BfInstruction::Right),
            4 => Some(BfInstruction::LoopStart),
            5 => Some(BfInstruction::LoopEnd),
            6 => Some(BfInstruction::Input),
            7 => Some(BfInstruction::Output),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

impl Display for BfInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let symbol = match self {
            BfInstruction::Inc => '+',
            BfInstruction::Dec => '-',
            BfInstruction::Left => '<',
            BfInstruction::Right => '>',
            BfInstruction::LoopStart => '[',
            BfInstruction::LoopEnd => ']',
            BfInstruction::Input => ',',
            BfInstruction::Output => '.',
        };
        write!(f, "{}", symbol)
    }
}

#[derive(Debug)]
pub struct CompressedBF {
    data: Vec<u8>,
    size: usize,
}

impl CompressedBF {
    pub(crate) fn iter(&self) -> impl Iterator<Item = BfInstruction> {
        (0..self.size).filter_map(move |i| self.get(i))
    }
}

impl CompressedBF {
    pub(crate) fn from_string<T: AsRef<str>>(p0: T) -> CompressedBF {
        let mut bf = CompressedBF::new(0, 0);
        for c in p0.as_ref().chars() {
            let instruction = match c {
                '+' => BfInstruction::Inc,
                '-' => BfInstruction::Dec,
                '<' => BfInstruction::Left,
                '>' => BfInstruction::Right,
                '[' => BfInstruction::LoopStart,
                ']' => BfInstruction::LoopEnd,
                ',' => BfInstruction::Input,
                '.' => BfInstruction::Output,
                _ => continue, // Ignore unknown characters
            };
            bf.append(instruction);
        }
        bf
    }
}

impl CompressedBF {
    pub fn new(size: usize, capacity: usize) -> CompressedBF {
        if capacity < size {
            panic!(
                "Capacity of {} must be greater than or equal to size {}",
                capacity, size
            );
        }
        let required_bytes = capacity.div_ceil(2);
        CompressedBF {
            data: vec![0u8; required_bytes],
            size,
        }
    }

    pub fn get(&self, index: usize) -> Option<BfInstruction> {
        if index >= self.size {
            return None;
        }
        let byte_pos = index / 2;
        let is_high = index % 2 == 1;
        let byte = self.data[byte_pos];
        let value = if is_high {
            (byte >> 4) & 0x0F
        } else {
            byte & 0x0F
        };
        BfInstruction::from_u8(value)
    }

    pub fn set(&mut self, index: usize, value: BfInstruction) {
        if index >= self.size {
            panic!("Index out of bounds: index {} >= size {}", index, self.size);
        }
        let byte_pos = index / 2;
        let is_high = index % 2 == 1;
        let val = value.to_u8() & 0x0F;
        if is_high {
            self.data[byte_pos] = (self.data[byte_pos] & 0x0F) | (val << 4);
        } else {
            self.data[byte_pos] = (self.data[byte_pos] & 0xF0) | val;
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn append(&mut self, value: BfInstruction) {
        let required_bytes = (self.size + 1).div_ceil(2);
        if self.data.len() < required_bytes {
            self.data.resize(required_bytes, 0);
        }
        self.size += 1;
        self.set(self.size - 1, value);
    }

    pub fn to_vec(&self) -> Vec<BfInstruction> {
        let mut res = Vec::new();
        for i in 0..self.size {
            res.push(self.get(i).unwrap());
        }
        res
    }
}

impl Clone for CompressedBF {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            size: self.size,
        }
    }
}

impl Display for CompressedBF {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        for i in 0..self.size {
            s.push(match self.get(i) {
                Some(BfInstruction::Inc) => '+',
                Some(BfInstruction::Dec) => '-',
                Some(BfInstruction::Left) => '<',
                Some(BfInstruction::Right) => '>',
                Some(BfInstruction::LoopStart) => '[',
                Some(BfInstruction::LoopEnd) => ']',
                Some(BfInstruction::Input) => ',',
                Some(BfInstruction::Output) => '.',
                None => '?', // Placeholder for invalid instruction
            });
        }
        write!(f, "{}", s)
    }
}

impl PartialEq for CompressedBF {
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size {
            return false;
        }
        for i in 0..self.size {
            if self.get(i) != other.get(i) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bf_instruction_from_to_and_display() {
        let symbols = ['+', '-', '<', '>', '[', ']', ',', '.'];
        for (i, &sym) in symbols.iter().enumerate() {
            let as_u8 = i as u8;
            let inst = BfInstruction::from_u8(as_u8).expect("from_u8 should return Some");
            assert_eq!(inst.to_u8(), as_u8);
            assert_eq!(format!("{}", inst), sym.to_string());
        }
        // invalid value returns None
        assert!(BfInstruction::from_u8(255).is_none());
    }

    #[test]
    fn test_compressedbf_append_get_set_size_to_vec_and_display() {
        let mut bf = CompressedBF::new(0, 0);
        assert_eq!(bf.size(), 0);

        bf.append(BfInstruction::Inc);
        bf.append(BfInstruction::Dec);
        bf.append(BfInstruction::Left);
        bf.append(BfInstruction::Right);

        assert_eq!(bf.size(), 4);
        assert_eq!(bf.get(0), Some(BfInstruction::Inc));
        assert_eq!(bf.get(1), Some(BfInstruction::Dec));
        assert_eq!(bf.get(2), Some(BfInstruction::Left));
        assert_eq!(bf.get(3), Some(BfInstruction::Right));

        // set and verify
        bf.set(1, BfInstruction::Output);
        assert_eq!(bf.get(1), Some(BfInstruction::Output));

        let vec = bf.to_vec();
        assert_eq!(vec, vec![BfInstruction::Inc, BfInstruction::Output, BfInstruction::Left, BfInstruction::Right]);

        // Display produces corresponding symbols
        assert_eq!(format!("{}", bf), "+.<>".to_string());
    }

    #[test]
    fn test_compressedbf_from_string_iter_and_to_vec() {
        let s = "+-<>[],.";
        let bf = CompressedBF::from_string(s);
        assert_eq!(bf.size(), 8);
        let collected: String = bf.iter().map(|i| format!("{}", i)).collect();
        assert_eq!(collected, s.to_string());
        assert_eq!(bf.to_vec().len(), 8);
    }

    #[test]
    fn test_iter_collects_correctly() {
        let mut bf = CompressedBF::new(0, 0);
        bf.append(BfInstruction::Inc);
        bf.append(BfInstruction::Dec);
        bf.append(BfInstruction::Output);

        let collected: Vec<BfInstruction> = bf.iter().collect();
        assert_eq!(collected, vec![BfInstruction::Inc, BfInstruction::Dec, BfInstruction::Output]);
    }

    #[test]
    fn test_clone_and_partial_eq() {
        let mut a = CompressedBF::new(0, 0);
        a.append(BfInstruction::Inc);
        a.append(BfInstruction::Dec);
        let b = a.clone();
        assert_eq!(a, b);
        // modify a and ensure inequality
        a.set(0, BfInstruction::Output);
        assert_ne!(a, b);
    }

    #[test]
    #[should_panic]
    fn test_new_capacity_less_than_size_panics() {
        // capacity < size should panic
        let _ = CompressedBF::new(5, 4);
    }

    #[test]
    #[should_panic]
    fn test_set_out_of_bounds_panics() {
        let mut bf = CompressedBF::new(1, 1);
        // valid indices: 0 only. Setting index 1 must panic.
        bf.set(1, BfInstruction::Inc);
    }

    #[test]
    fn test_get_out_of_range_returns_none() {
        let bf = CompressedBF::new(1, 1);
        assert_eq!(bf.get(1), None);
    }
}
