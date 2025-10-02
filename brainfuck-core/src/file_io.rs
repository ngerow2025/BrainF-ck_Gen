use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    sync::{
        Arc, Mutex,
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
};

use crate::{
    data::{BfInstruction, CompressedBF},
    run::{ContinueState, ProgramState, RunningProgramInfo},
};

pub struct DiskSeedWriter<const MAX_TAPE_SIZE: usize> {
    sender: Option<Sender<RunningProgramInfo<MAX_TAPE_SIZE>>>,
    handle: Option<JoinHandle<()>>,
    file: Arc<Mutex<BufWriter<File>>>,
    program_size: usize,
}

impl<const MAX_TAPE_SIZE: usize> DiskSeedWriter<MAX_TAPE_SIZE> {
    pub fn new(program_size: usize) -> Self {
        let file_path = format!("program_{}_seeds_{}.bin", MAX_TAPE_SIZE, program_size);
        let file = match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
        {
            std::result::Result::Ok(file) => file,
            std::result::Result::Err(err) => {
                panic!("Could not open file for writing: {}", err);
            }
        };

        Self::with_file(program_size, file)
    }

    fn with_file(program_size: usize, file: File) -> DiskSeedWriter<MAX_TAPE_SIZE> {
        let mut file = BufWriter::with_capacity(1_000_000_000, file);
        file.write_all(&program_size.to_ne_bytes()).unwrap();

        let file = Arc::new(Mutex::new(file));
        let (sender, receiver) = mpsc::channel::<RunningProgramInfo<MAX_TAPE_SIZE>>();
        let file_clone = Arc::clone(&file);

        let handle = thread::spawn(move || {
            for program in receiver {
                let mut file = file_clone.lock().unwrap();

                // write code
                let code_bytes = program
                    .code
                    .to_vec()
                    .iter()
                    .map(|b| (*b).to_u8())
                    .collect::<Vec<u8>>();
                file.write_all(&code_bytes)
                    .expect("Could not write program code");

                // write jump table
                let jump_table_bytes = program
                    .jump_table
                    .iter()
                    .flat_map(|&x| x.to_ne_bytes())
                    .collect::<Vec<u8>>();
                file.write_all(&jump_table_bytes)
                    .expect("Could not write jump table");

                file.write_all(&program.continue_state.program_state.tape)
                    .expect("Could not write tape");
                file.write_all(&[program.continue_state.program_state.tape_head])
                    .expect("Could not write tape head");
                file.write_all(&program.continue_state.resume_pc.to_ne_bytes())
                    .expect("Could not write pc");
                file.write_all(&program.continue_state.resume_output_ind.to_ne_bytes())
                    .expect("Could not write output index");

                // write paren count
                file.write_all(&program.current_paren_count.to_ne_bytes())
                    .expect("Could not write paren count");
            }
        });

        DiskSeedWriter {
            sender: Some(sender),
            handle: Some(handle),
            file,
            program_size,
        }
    }

    pub fn append(&mut self, program: RunningProgramInfo<MAX_TAPE_SIZE>) {
        if program.code.size() != self.program_size {
            panic!(
                "Program size mismatch: {} != {}",
                program.code.size(),
                self.program_size
            );
        }

        if let Some(sender) = &self.sender {
            sender
                .send(program)
                .expect("Failed to send program to worker thread");
        }
    }

    pub fn flush(&mut self) {
        // Drop sender so the worker thread knows there’s nothing more
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .expect("Failed to join worker thread of DiskSeedWriter");
        }

        let mut file = self.file.lock().unwrap();
        file.flush().expect("Failed to flush file");
    }
}

pub struct DiskSeedReader<const MAX_TAPE_SIZE: usize> {
    file: BufReader<File>,
    program_size: usize,
}

impl<const MAX_TAPE_SIZE: usize> DiskSeedReader<MAX_TAPE_SIZE> {
    pub fn new(program_size: usize) -> Self {
        //make sure the file exists
        let file_path = format!("program_seeds_{}.bin", program_size);
        let file = OpenOptions::new()
            .read(true)
            .open(file_path)
            .expect("Could not open file for reading");
        Self::with_file(program_size, file)
    }

    pub fn with_file(program_size: usize, file: File) -> DiskSeedReader<MAX_TAPE_SIZE> {
        let mut file = BufReader::with_capacity(1_000_000_000, file);

        let mut size_bytes = [0u8; usize::to_ne_bytes(0).len()];
        file.read_exact(&mut size_bytes)
            .expect("Could not read program size from file");
        let read_program_size = usize::from_ne_bytes(size_bytes);
        if program_size != read_program_size {
            panic!(
                "Program size does not match expected size: {} != {}",
                program_size, read_program_size
            );
        }

        DiskSeedReader { file, program_size }
    }

    pub fn read_seed(&mut self) -> Option<RunningProgramInfo<MAX_TAPE_SIZE>> {
        let mut code = CompressedBF::new(self.program_size, self.program_size + 1);
        let mut jump_table = Vec::with_capacity(self.program_size + 1);

        // Read program code
        let mut code_bytes = vec![0u8; self.program_size];
        if self.file.read_exact(&mut code_bytes).is_err() {
            return None; // End of file or read error
        }
        for (i, byte) in code_bytes.iter().enumerate() {
            if let Some(instruction) = BfInstruction::from_u8(*byte) {
                code.set(i, instruction);
            } else {
                return None; // Invalid instruction
            }
        }

        // Read jump table
        let jump_table_size = self.program_size;
        //read jump_table_size * sizeof(i64) bytes
        let mut jump_table_bytes = vec![0u8; jump_table_size * std::mem::size_of::<i64>()];
        if self.file.read_exact(&mut jump_table_bytes).is_err() {
            return None; // End of file or read error
        }
        for i in 0..jump_table_size {
            let start = i * std::mem::size_of::<i64>();
            let end = start + std::mem::size_of::<i64>();
            let jump_value = i64::from_ne_bytes(jump_table_bytes[start..end].try_into().unwrap());
            jump_table.push(jump_value);
        }

        //read the MAX_TAPE_SIZE bytes of tape
        let mut tape = [0u8; MAX_TAPE_SIZE];
        if self.file.read_exact(&mut tape).is_err() {
            return None; // End of file or read error
        }
        //read the tape head
        let mut tape_head_bytes = [0u8; 1];
        if self.file.read_exact(&mut tape_head_bytes).is_err() {
            return None; // End of file or read error
        }
        let tape_head = tape_head_bytes[0];

        //read the program counter
        let mut pc_bytes = [0u8; std::mem::size_of::<usize>()];
        if self.file.read_exact(&mut pc_bytes).is_err() {
            return None; // End of file or read error
        }
        let pc = usize::from_ne_bytes(pc_bytes);

        // Read the output index
        let mut output_index_bytes = [0u8; std::mem::size_of::<usize>()];
        if self.file.read_exact(&mut output_index_bytes).is_err() {
            return None; // End of file or read error
        }
        let output_index = usize::from_ne_bytes(output_index_bytes);

        let continue_state = ContinueState {
            program_state: ProgramState { tape, tape_head },
            resume_pc: pc,
            resume_output_ind: output_index,
        };

        // Read paren count
        let mut paren_count_bytes = [0u8; std::mem::size_of::<usize>()];
        if self.file.read_exact(&mut paren_count_bytes).is_err() {
            return None; // End of file or read error
        }
        let current_paren_count = usize::from_ne_bytes(paren_count_bytes);

        Some(RunningProgramInfo {
            code,
            jump_table,
            continue_state,
            current_paren_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Seek;

    use tempfile::tempfile;

    use super::*;
    use crate::data::BfInstruction;

    const TEST_TAPE_SIZE: usize = 32;
    const TEST_PROGRAM_SIZE: usize = 8;

    fn create_dummy_program(seed: u8) -> RunningProgramInfo<TEST_TAPE_SIZE> {
        let mut code = CompressedBF::new(TEST_PROGRAM_SIZE, TEST_PROGRAM_SIZE);
        for i in 0..TEST_PROGRAM_SIZE {
            code.set(i, BfInstruction::from_u8((i as u8 + seed) % 8).unwrap());
        }

        let jump_table = (0..TEST_PROGRAM_SIZE as i64)
            .map(|i| i * 2 - seed as i64)
            .collect();

        let mut tape = [0; TEST_TAPE_SIZE];
        for (i, item) in tape.iter_mut().enumerate() {
            *item = (i as u8 + seed) % 255;
        }

        RunningProgramInfo {
            code,
            jump_table,
            continue_state: ContinueState {
                program_state: ProgramState {
                    tape,
                    tape_head: seed % TEST_TAPE_SIZE as u8,
                },
                resume_pc: seed as usize,
                resume_output_ind: (seed + 1) as usize,
            },
            current_paren_count: (seed + 2) as usize,
        }
    }

    #[test]
    fn test_write_and_read_single_seed() {
        let program = create_dummy_program(42);

        // Use a tempfile and the with_file constructor so the test doesn't create
        // a named file on disk.
        let mut file = tempfile().unwrap();

        {
            let mut writer = DiskSeedWriter::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file.try_clone().unwrap());
            writer.append(program.clone());
            writer.flush();
        }

        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let mut reader = DiskSeedReader::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file);
        let read_program = reader.read_seed().expect("Should be able to read one seed");

        assert_eq!(
            program, read_program,
            "The written and read programs should be identical"
        );

        // Ensure no more data is in the file
        assert!(
            reader.read_seed().is_none(),
            "There should be no more seeds to read"
        );
    }

    #[test]
    fn test_write_and_read_multiple_seeds() {
        let programs = (0..5).map(create_dummy_program).collect::<Vec<_>>();

        let mut file = tempfile().unwrap();

        {
            let mut writer = DiskSeedWriter::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file.try_clone().unwrap());
            for program in &programs {
                writer.append(program.clone());
            }
            writer.flush();
        }

        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let mut reader = DiskSeedReader::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file);
        for original_program in programs {
            let read_program = reader.read_seed().expect("Should read a seed");
            assert_eq!(original_program, read_program);
        }

        assert!(
            reader.read_seed().is_none(),
            "Should be at the end of the file"
        );
    }

    #[test]
    #[should_panic(expected = "Program size mismatch")]
    fn test_writer_panic_on_size_mismatch() {
    let file = tempfile().unwrap();
    let mut writer = DiskSeedWriter::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file);
        let wrong_size_program = RunningProgramInfo {
            code: CompressedBF::new(TEST_PROGRAM_SIZE + 1, TEST_PROGRAM_SIZE + 1),
            jump_table: vec![],
            continue_state: create_dummy_program(0).continue_state,
            current_paren_count: 0,
        };
        writer.append(wrong_size_program);
        // The panic should occur on append, but we flush to ensure the test fails if it doesn't
        writer.flush();
    }

    #[test]
    #[should_panic(expected = "Program size does not match expected size")]
    fn test_reader_panic_on_header_mismatch() {
        let program = create_dummy_program(1);
        let mut file = tempfile().unwrap();

        {
            let mut writer = DiskSeedWriter::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file.try_clone().unwrap());
            writer.append(program);
            writer.flush();
        }

        // Try to read with the wrong program size
        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let _reader = DiskSeedReader::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE + 1, file);
    }

    #[test]
    #[should_panic]
    fn test_read_from_empty_file_panics() {
        // Create a tempfile and write only the header
        let mut file = tempfile().unwrap();
        {
            let _writer = DiskSeedWriter::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file.try_clone().unwrap());
        }

        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let mut reader = DiskSeedReader::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file);
        assert!(
            reader.read_seed().is_none(),
            "Reading from a file with only a header should return None"
        );
    }

    #[test]
    fn test_read_from_corrupted_file_returns_none() {
        let program = create_dummy_program(10);
        let mut file = tempfile().unwrap();

        {
            let mut writer = DiskSeedWriter::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file.try_clone().unwrap());
            writer.append(program);
            writer.flush();
        }

        // Manually truncate the file to make it corrupted
        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        // Truncate to a size that's partway through the written data
        file.set_len(std::mem::size_of::<usize>() as u64 + 20)
            .unwrap();

        file.seek(std::io::SeekFrom::Start(0)).unwrap();
        let mut reader = DiskSeedReader::<TEST_TAPE_SIZE>::with_file(TEST_PROGRAM_SIZE, file);
        assert!(
            reader.read_seed().is_none(),
            "Reading from a truncated/corrupted file should return None"
        );
    }
}
