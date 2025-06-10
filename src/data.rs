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
    pub(crate) fn from_string(p0: String) -> CompressedBF {
        let size = p0.len();
        let mut bf = CompressedBF::new(0, 0);
        for c in p0.chars() {
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
            panic!("Capacity must be greater than or equal to size");
        }
        let required_bytes = (capacity + 1) / 2;
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
            panic!("Index out of bounds");
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
        let required_bytes = ((self.size + 1) + 1) / 2;
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

    pub fn to_string(&self) -> String {
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
        s
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
        let instructions: Vec<String> = (0..self.size)
            .filter_map(|i| self.get(i).map(|instr| instr.to_string()))
            .collect();
        write!(f, "{}", instructions.join(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_size() {
        let bf = CompressedBF::new(10, 10);
        assert_eq!(bf.size(), 10);
        assert!(bf.to_vec().iter().all(|&x| x == BfInstruction::Inc)); // default is not initialized
    }

    #[test]
    fn test_set_and_get() {
        let mut bf = CompressedBF::new(5, 5);
        let sequence = [
            BfInstruction::Inc,
            BfInstruction::Dec,
            BfInstruction::Left,
            BfInstruction::Right,
            BfInstruction::LoopStart,
        ];
        for (i, &instr) in sequence.iter().enumerate() {
            bf.set(i, instr);
        }
        for (i, &instr) in sequence.iter().enumerate() {
            assert_eq!(bf.get(i), Some(instr));
        }
    }

    #[test]
    #[should_panic(expected = "Index out of bounds")]
    fn test_set_out_of_bounds_panics() {
        let mut bf = CompressedBF::new(3, 3);
        bf.set(4, BfInstruction::Input); // should panic
    }

    #[test]
    fn test_append_and_get() {
        let mut bf = CompressedBF::new(0, 0);
        let sequence = [
            BfInstruction::Input,
            BfInstruction::Output,
            BfInstruction::LoopEnd,
            BfInstruction::LoopStart,
        ];
        for &instr in &sequence {
            bf.append(instr);
        }
        assert_eq!(bf.size(), sequence.len());
        for (i, &instr) in sequence.iter().enumerate() {
            assert_eq!(bf.get(i), Some(instr));
        }
    }

    #[test]
    fn test_to_vec() {
        let mut bf = CompressedBF::new(3, 3);
        bf.set(0, BfInstruction::Left);
        bf.set(1, BfInstruction::Right);
        bf.set(2, BfInstruction::Dec);
        assert_eq!(
            bf.to_vec(),
            vec![
                BfInstruction::Left,
                BfInstruction::Right,
                BfInstruction::Dec
            ]
        );
    }

    #[test]
    fn test_to_string() {
        let mut bf = CompressedBF::new(4, 4);
        bf.set(0, BfInstruction::Inc);
        bf.set(1, BfInstruction::Dec);
        bf.set(2, BfInstruction::LoopStart);
        bf.set(3, BfInstruction::LoopEnd);
        assert_eq!(bf.to_string(), "+-[]");
    }

    #[test]
    fn test_clone() {
        let mut bf = CompressedBF::new(2, 2);
        bf.set(0, BfInstruction::Input);
        bf.set(1, BfInstruction::Output);
        let clone = bf.clone();
        assert_eq!(clone.get(0), Some(BfInstruction::Input));
        assert_eq!(clone.get(1), Some(BfInstruction::Output));
        assert_eq!(clone.size(), bf.size());
        assert_eq!(clone.to_vec(), bf.to_vec());
    }
}
