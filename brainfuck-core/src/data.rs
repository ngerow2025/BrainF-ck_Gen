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
