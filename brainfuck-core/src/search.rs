#![allow(dead_code)]

use ahash::{HashSet, RandomState};

use crate::{
    data::{BfInstruction, CompressedBF}, file_io::{DiskSeedReader, DiskSeedWriter}, run::{
        get_max_steps_reached, run_program_fragment, run_program_fragment_without_states, BfRunResult, ContinueState, ProgramState, RunningProgramInfo
    }
};

fn find_program<const MAX_TAPE_SIZE: usize>(
    target_output: &[u8],
    starting_program: String,
) -> Result<Vec<BfInstruction>, &'static str> {
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
        return Err("Starting program has unmatched parentheses.");
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
                    return Err("Loop end without matching loop start.");
                }
            }
            _ => jump_table.push(-1), // -1 indicates non-loop instruction
        }
    }

    // construct RunningProgramInfo for the starting program
    let starting_program_info = RunningProgramInfo {
        code: starting_program.clone(),
        current_paren_count: 0,
        jump_table,
        continue_state: ContinueState {
            resume_pc: 0,
            resume_output_ind: 0,
            program_state: ProgramState {
                tape: [0u8; MAX_TAPE_SIZE],
                tape_head: 0,
            },
        },
    };

    //run initial program
    let initial_program_run_result = run_program_fragment(&starting_program_info, target_output);
    println!("Program run result: {:?}", initial_program_run_result);
    let mut found_states = HashSet::with_capacity_and_hasher(5_000_000, RandomState::default());

    handle_run_result(
        initial_program_run_result,
        starting_program_info,
        &mut current_program_writing_head,
        &mut found_states,
    );

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
            if (program_seed.code.size() == 0
                || program_seed.code.get(program_seed.code.size() - 1)
                    != Some(BfInstruction::LoopStart))
                && (program_seed.current_paren_count > 0)
            {
                //loop end instruction
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::LoopEnd);

                //add the newly completed loop into the jump table
                let loop_start_loc = program_seed
                    .jump_table
                    .iter()
                    .rposition(|x| *x == -2)
                    .unwrap();
                new_program.jump_table[loop_start_loc] = new_program.code.size() as i64;
                new_program.jump_table.push((loop_start_loc + 1) as i64);
                new_program.current_paren_count -= 1;

                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
            //loop start instruction
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::LoopStart);
                new_program.current_paren_count += 1;
                //add a -2 to the jump table to mark the start of the loop
                new_program.jump_table.push(-2);
                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
            //output instruction
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Output);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
            //left instruction
            if program_seed.code.size() == 0
                || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Right)
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Left);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
            //right instruction
            if program_seed.code.size() == 0
                || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Left)
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Right);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
            //increment instruction
            if program_seed.code.size() == 0
                || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Dec)
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Inc);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
            //decrement instruction
            if program_seed.code.size() == 0
                || program_seed.code.get(program_seed.code.size() - 1) != Some(BfInstruction::Inc)
            {
                let mut new_program = program_seed.clone();
                new_program.code.append(BfInstruction::Dec);
                new_program.jump_table.push(-1); // -1 indicates non-loop instruction
                let run_res = run_program_fragment_without_states(&new_program, target_output);
                if let Some(working_program) = handle_run_result(
                    run_res,
                    new_program,
                    &mut current_program_writing_head,
                    &mut found_states,
                ) {
                    return Ok(working_program);
                }
            }
        }
        current_program_writing_head.flush();

        println!(
            "Finished processing all programs of size {}. Max steps reached: {}",
            current_program_size - 1,
            get_max_steps_reached()
        );

        if current_program_size == 16 {
            return Err("Reached maximum program size of 16 without finding a solution.");
        }

        println!(
            "Advanced to next layer of program size of {}.",
            current_program_size
        );

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
}

fn handle_run_result<const MAX_TAPE_SIZE: usize>(
    run_res: BfRunResult<MAX_TAPE_SIZE>,
    mut new_program: RunningProgramInfo<MAX_TAPE_SIZE>,
    new_programs: &mut DiskSeedWriter<MAX_TAPE_SIZE>,
    found_states: &mut HashSet<(ProgramState<MAX_TAPE_SIZE>, usize)>,
) -> Option<Vec<BfInstruction>> {
    match run_res {
        BfRunResult::IncompleteLoopSuccess(continue_state) => {
            new_program.continue_state = continue_state;
            new_programs.append(new_program.clone());
            None
        }
        BfRunResult::Success => Some(new_program.code.to_vec()),
        BfRunResult::IncompleteOutputSuccess(end_state) => {
            if found_states
                .contains(&(end_state.program_state.clone(), end_state.resume_output_ind))
            {
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
