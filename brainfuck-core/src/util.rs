//convience functions to hide implemtation details better

use crate::{data::{BfInstruction, CompressedBF}, run::{run_program_fragment, run_program_fragment_no_target, ContinueState, ProgramState, RunningProgramInfo}};
// TODO: Do actual error types instead of hamfisted &'static str
pub fn preprocess_input(input: &str) -> Result<RunningProgramInfo<30_000>, &'static str> {
    let program_code = CompressedBF::from_string(&input);


    let continue_state = ContinueState {
        resume_pc: 0,
        resume_output_ind: 0,
        program_state: ProgramState {
            tape: [0; 30_000],
            tape_head: 0,
        }
    };

    let mut jump_table = vec![0; program_code.size()];

    let mut current_paren_count = 0;

    for i in 0..program_code.size() {
        match program_code.get(i) {
            Some(BfInstruction::LoopStart) => {
                jump_table.push(-2);
                current_paren_count += 1;
            }
            Some(BfInstruction::LoopEnd) => {
                //find the last -2 in the jump table and set it to the current index + 1 and append the index of the loop start + 1
                if let Some(loop_start_index) = jump_table.iter().rposition(|&x| x == -2) {
                    jump_table[loop_start_index] = i as i64 + 1; // set the loop start to the current index + 1
                    jump_table.push((loop_start_index + 1) as i64); // append the index of the loop start + 1
                    current_paren_count -= 1;
                } else {
                    return Err("Loop end without matching loop start.");
                }
            }
            _ => jump_table.push(-1), // -1 indicates non-loop instruction
        }
    }

    if current_paren_count != 0 {
        return Err("Unmatched loop in the input.");
    }

    Ok(RunningProgramInfo { code: program_code, current_paren_count, jump_table, continue_state })
}



// pub fn run_program(input: RunningProgramInfo<30_000>) {
//     let internal_res = run_program_fragment_no_target(input.code, );
// }