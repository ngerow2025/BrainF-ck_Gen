//look at smallvec to get rid of heap allocation, they are destroying performance in the hot path
//add additional logic for jump tables to handle repeated loop closes and starts better

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use crate::data::{BfInstruction, CompressedBF};
use crate::run::{run_program_fragment, BfRunResult, ContinueState, ProgramState, RunningProgramInfo, HASHSET_SIZE_HISTOGRAM};
use std::{fs, fs::{File, OpenOptions}, io::{BufReader, BufWriter, Seek, SeekFrom}, path::PathBuf};
use std::marker::PhantomData;
use ahash::RandomState;

const MAX_TAPE_SIZE: usize = 5;
mod data;
mod run;

fn main() {
    let target_output: &[u8] = &[15u8]; // Example target output

    //+++[>+++++<-]
    HASHSET_SIZE_HISTOGRAM.get_or_init(|| Mutex::new(HashMap::new()));


    let program = find_program(target_output, "".to_string());

    let hist = HASHSET_SIZE_HISTOGRAM.get().unwrap().lock().unwrap();
    println!("HashSet final size histogram:");
    let mut sorted: Vec<_> = hist.iter().collect();
    sorted.sort_by_key(|&(k, _)| *k);
    for (size, count) in sorted {
        println!("Size {}: {}", size, count);
    }
    
    if program.is_empty() {
        println!("No program found to produce the target output.");
    } else {
        println!("Found program: {:?}", program);
        // Optionally, you can write the program to a file or execute it.
    }
}

fn find_program(target_output: &[u8], starting_program: String) -> Vec<BfInstruction> {
    //parse the starting program
    let starting_program = CompressedBF::from_string(starting_program);
    
    let mut current_program_size = starting_program.size();
    let mut current_program_writing_head = DiskSeedWriter::new(current_program_size);
    
    // calculate and check paren_count
    let mut paren_count = 0;
    for instruction in starting_program.iter() {
        match instruction {
            BfInstruction::LoopStart => paren_count += 1,
            BfInstruction::LoopEnd => paren_count -= 1,
            _ => {}
        }
    }
    if paren_count != 0 {
        panic!("Starting program has unmatched parentheses.");
    }
    
    // construct the jump table
    let mut jump_table = Vec::with_capacity(starting_program.size() + 1);
    for i in 0..starting_program.size() {
        match starting_program.get(i) {
            Some(BfInstruction::LoopStart) => jump_table.push(-2), // -2 indicates start of loop
            Some(BfInstruction::LoopEnd) => { 
                //find the last -2 in the jump table and set it to the current index + 1 and append the index of the loop start + 1
                if let Some(loop_start_index) = jump_table.iter().rposition(|&x| x == -2) {
                    jump_table[loop_start_index] = i as i64 + 1; // set the loop start to the current index + 1
                    jump_table.push((loop_start_index + 1) as i64); // append the index of the loop start + 1
                } else {
                    panic!("Loop end without matching loop start.");
                }
            }, 
            _ => jump_table.push(-1), // -1 indicates non-loop instruction
        }
    }
    
    
    
    // construct RunningProgramInfo for the starting program
    let starting_program_info = RunningProgramInfo {
        code: starting_program.clone(),
        current_paren_count: 0,
        jump_table,
        continue_state: ContinueState {
            program_state: ProgramState {
                tape: [0u8; MAX_TAPE_SIZE],
                tape_head: 0,
            },
            resume_pc: 0,
            resume_output_ind: 0,
        },
    };
    

    //run initial program
    let initial_program_run_result = run_program_fragment(&starting_program_info, target_output);
    println!("Program run result: {:?}", initial_program_run_result);
    let mut found_states = HashSet::with_capacity_and_hasher(5_000_000, RandomState::default());
    
    handle_run_result(initial_program_run_result, starting_program_info, &mut current_program_writing_head, &mut found_states);
    
    current_program_writing_head.flush();
    
    let mut current_program_reading_head;

    loop {
        current_program_writing_head.flush();
        current_program_writing_head = DiskSeedWriter::new(current_program_size + 1);
        current_program_reading_head = DiskSeedReader::new(current_program_size);
        
        current_program_size += 1;

        // if current_program_size == 12 {
        //     return vec![];
        // }
        


        while let Some(program_seed) = current_program_reading_head.read_seed() {
            if (program_seed.code.size() == 0 || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::LoopStart)) && (program_seed.current_paren_count > 0) {
                //loop end instruction
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::LoopEnd);
                
                //add the newly completed loop into the jump table
                let loop_start_loc = program_seed.jump_table.iter().rposition(|x| {*x == -2}).unwrap();
                new_program.jump_table[loop_start_loc] = new_program.code.size() as i64;
                new_program.jump_table.push((loop_start_loc + 1) as i64);
                new_program.current_paren_count -= 1;

                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
            //loop start instruction
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::LoopStart);
                new_program.current_paren_count += 1;
                //add a -2 to the jump table to mark the start of the loop
                new_program.jump_table.push(-2);
                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
            //output instruction
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Output);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
            //left instruction
            if program_seed.code.size() == 0 || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Right) {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Left);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
            //right instruction
            if program_seed.code.size() == 0 || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Left) {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Right);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
            //increment instruction
            if program_seed.code.size() == 0 || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Dec) {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Inc);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
            //decrement instruction
            if program_seed.code.size() == 0 || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Inc) {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Dec);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment(&new_program, target_output);
                if let Some(working_program) = handle_run_result(run_res, new_program, &mut current_program_writing_head, &mut found_states) {
                    return working_program;
                }
            }
        }
        current_program_writing_head.flush();
        
        if current_program_size == 13 {
            return vec![]; // Stop condition for testing purposes
        }

        println!("Advanced to next layer of program size of {}.", current_program_size);


        //debug write all of the current programs to file and their jump table, code, and continue state to a file
        // let file_name = format!("program_{}.bf", program_seeds.iter().last().unwrap().code.size());
        // let mut file = std::fs::File::create(file_name).expect("Could not create file");
        // for program_seed in program_seeds.iter() {
        //     let code_str = program_seed.code.to_string();
        //     let jump_table_strs = program_seed.jump_table.iter()
        //         .map(|x| x.to_string())
        //         .collect::<Vec<String>>();
        //     let continue_state_str = if let Some(real_continue_state) = &program_seed.continue_state { format!(
        //         "Tape: {:?}, Tape Head: {}, pc: {}",
        //         real_continue_state.0.tape,
        //         real_continue_state.0.tape_head,
        //         real_continue_state.1
        //     )} else {
        //         "No continue state".to_string()
        //     };
        //     
        //     //output the code with 2 spaces in between each instruction
        //     //outupt the jump table under it aligned with the code
        //     //finally output the continue state
        //     for char in code_str.chars() {
        //         if char != ' ' {
        //             file.write_all(format!("{}  ", char).as_bytes()).expect("Could not write to file");
        //         }
        //     }
        //     file.write_all(b"\n").expect("Could not write to file");
        //     //make sure that each jump table entry is 3 characters wide
        //     for jump in jump_table_strs {
        //         file.write_all(format!("{:>3}", jump).as_bytes()).expect("Could not write to file");
        //     }
        //     file.write_all(b"\n").expect("Could not write to file");
        //     file.write_all(continue_state_str.as_bytes()).expect("Could not write to file");
        //     file.write_all(b"\n").expect("Could not write to file");
        // }
        //debug print the number of programs
                
    }


    Vec::new()
}

fn handle_run_result(
    run_res: BfRunResult,
    mut new_program: RunningProgramInfo,
    new_programs: &mut DiskSeedWriter,
    found_states: &mut HashSet<(ProgramState, usize), RandomState>,
) -> Option<Vec<BfInstruction>> {
    match run_res {
        BfRunResult::IncompleteLoopSuccess(continue_state) => {
            new_program.continue_state = continue_state;
            new_programs.append(new_program.clone());
            None
        }
        BfRunResult::Success => {
            Some(new_program.code.to_vec())
        }
        BfRunResult::IncompleteOutputSuccess(end_state) => {
            if found_states.contains(&(end_state.program_state.clone(), end_state.resume_output_ind)) {
                return None; // Skip already found state
            } else {
                found_states.insert((end_state.program_state.clone(), end_state.resume_output_ind));
                // println!("total found states: {}", found_states.len());
            }
            new_program.continue_state = end_state;
            new_programs.append(new_program.clone());
            None
        }
        _ => None,
    }
}

pub struct DiskSeedWriter {
    sender: Option<Sender<RunningProgramInfo>>,
    handle: Option<JoinHandle<()>>,
    file: Arc<Mutex<BufWriter<File>>>,
    program_size: usize,
    _phantom: PhantomData<()>,
}

impl DiskSeedWriter {
    pub fn new(program_size: usize) -> Self {
        let file_path = format!("program_seeds_{}.bin", program_size);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file_path)
            .expect("Could not open file for writing");

        let mut file = BufWriter::with_capacity(1_000_000_000, file);
        file.write(&program_size.to_ne_bytes()).unwrap();

        let file = Arc::new(Mutex::new(file));
        let (sender, receiver) = mpsc::channel::<RunningProgramInfo>();
        let file_clone = Arc::clone(&file);

        let handle = thread::spawn(move || {
            for program in receiver {
                let mut file = file_clone.lock().unwrap();

                // write code
                let code_bytes = program.code.to_vec().iter().map(|b| (*b).to_u8()).collect::<Vec<u8>>();
                file.write_all(&code_bytes).expect("Could not write program code");

                // write jump table
                let jump_table_bytes = program.jump_table.iter().map(|&x| x.to_ne_bytes()).flatten().collect::<Vec<u8>>();
                file.write_all(&jump_table_bytes).expect("Could not write jump table");


                file.write_all(&program.continue_state.program_state.tape).expect("Could not write tape");
                file.write_all(&program.continue_state.program_state.tape_head.to_ne_bytes()).expect("Could not write tape head");
                file.write_all(&program.continue_state.resume_pc.to_ne_bytes()).expect("Could not write pc");
                file.write_all(&program.continue_state.resume_output_ind.to_ne_bytes()).expect("Could not write output index");

                // write paren count
                file.write_all(&program.current_paren_count.to_ne_bytes()).expect("Could not write paren count");
            }
        });

        DiskSeedWriter {
            sender: Some(sender),
            handle: Some(handle),
            file,
            program_size,
            _phantom: PhantomData,
        }
    }

    pub fn append(&mut self, program: RunningProgramInfo) {
        if program.code.size() != self.program_size {
            panic!("Program size mismatch: {} != {}", program.code.size(), self.program_size);
        }

        if let Some(sender) = &self.sender {
            sender.send(program).expect("Failed to send program to worker thread");
        }
    }

    pub fn flush(&mut self) {
        // Drop sender so the worker thread knows there’s nothing more
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            handle.join().expect("Worker thread panicked");
        }

        let mut file = self.file.lock().unwrap();
        file.flush().expect("Failed to flush file");
    }
}


pub struct DiskSeedReader {
    file: BufReader<File>,
    program_size: usize,
}

impl DiskSeedReader {
    pub fn new(program_size: usize) -> Self {
        //make sure the file exists
        let file_path = format!("program_seeds_{}.bin", program_size);
        let file = OpenOptions::new()
            .read(true)
            .open(file_path)
            .expect("Could not open file for reading");
        let mut file = BufReader::with_capacity(1_000_000_000, file);
        
        let mut size_bytes = [0u8; usize::to_ne_bytes(0).len()];
        file.read_exact(&mut size_bytes).expect("Could not read program size from file");
        let program_size = usize::from_ne_bytes(size_bytes);
        if program_size != program_size {
            panic!("Program size does not match expected size: {} != {}", program_size, program_size);
        }
        
        DiskSeedReader {
            file,
            program_size,
        }
    }
    
    pub fn read_seed(&mut self) -> Option<RunningProgramInfo> {
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
        let mut tape_head_bytes = [0u8; std::mem::size_of::<usize>()];
        if self.file.read_exact(&mut tape_head_bytes).is_err() {
            return None; // End of file or read error
        }
        let tape_head = usize::from_ne_bytes(tape_head_bytes);
        
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
