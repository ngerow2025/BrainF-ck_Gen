//convience functions to hide implemtation details better

use crate::{
    data::{BfInstruction, CompressedBF},
    run::{ContinueState, ProgramState, RunningProgramInfo},
};

// TODO: Do actual error types instead of hamfisted &'static str
pub fn preprocess_input<const MAX_TAPE_SIZE: usize>(input: &str) -> Result<RunningProgramInfo<MAX_TAPE_SIZE>, &'static str> {
    let program_code = CompressedBF::from_string(input);

    let continue_state = ContinueState {
        resume_pc: 0,
        resume_output_ind: 0,
        program_state: ProgramState {
            tape: [0; MAX_TAPE_SIZE],
            tape_head: 0,
        },
    };

    let mut jump_table = Vec::with_capacity(program_code.size());

    let mut current_paren_count = 0;

    for (i, instruction) in program_code.iter().enumerate() {
        match instruction {
            BfInstruction::LoopStart => {
                jump_table.push(-2);
                current_paren_count += 1;
            }
            BfInstruction::LoopEnd => {
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

    Ok(RunningProgramInfo {
        code: program_code,
        current_paren_count,
        jump_table,
        continue_state,
    })
}

// pub fn run_program(input: RunningProgramInfo<30_000>) {
//     let internal_res = run_program_fragment_no_target(input.code, );
// }

// --- UNIT TESTS ---
#[cfg(test)]
mod tests {
    use super::*;
    const TAPE_SIZE: usize = 30000;

    /// Tests preprocessing an empty Brainfuck program.
    #[test]
    fn test_empty_input() {
        let result = preprocess_input::<TAPE_SIZE>("");
        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.code.size(), 0);
        assert_eq!(info.jump_table, vec![]);
        assert_eq!(info.current_paren_count, 0);
    }

    /// Tests a program with valid instructions but no loops.
    #[test]
    fn test_no_loops() {
        let input = "+-<>,.";
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_ok());
        let info = result.unwrap();
        
        let expected_code = CompressedBF::from_string(input);
        assert_eq!(info.code, expected_code);
        
        // The jump table will contain 6 zeros from initialization and then 6 `-1`s pushed due to the bug.
        let expected_jump_table = vec![-1, -1, -1, -1, -1, -1];
        assert_eq!(info.jump_table, expected_jump_table);
        
        assert_eq!(info.current_paren_count, 0);
    }

    /// Tests a program with a single, simple loop.
    #[test]
    fn test_simple_loop() {
        let input = "+[]"; // 3 instructions
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_ok());
        let info = result.unwrap();

        
        let expected_jump_table = vec![-1, 3, 2];
        assert_eq!(info.jump_table, expected_jump_table);
        assert_eq!(info.current_paren_count, 0);
    }
    
    /// Tests a program with nested loops.
    #[test]
    fn test_nested_loops() {
        let input = "[[]]"; // 4 instructions
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_ok());
        let info = result.unwrap();
        
        let expected_jump_table = vec![4, 3, 2, 1];
        assert_eq!(info.jump_table, expected_jump_table);
        assert_eq!(info.current_paren_count, 0);
    }
    
    /// Tests for an unclosed loop start bracket '['.
    #[test]
    fn test_unmatched_loop_start() {
        let input = "[.";
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Unmatched loop in the input.");
    }

    /// Tests for a loop end bracket ']' without a matching start.
    #[test]
    fn test_unmatched_loop_end() {
        let input = ".]";
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Loop end without matching loop start.");
    }

    /// Tests for mismatched brackets where ']' appears before '['.
    #[test]
    fn test_mismatched_loops() {
        let input = "][.";
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Loop end without matching loop start.");
    }
    
    /// Verifies that the initial state of the program is set correctly.
    #[test]
    fn test_initial_continue_state() {
        let result = preprocess_input::<TAPE_SIZE>("+").unwrap();
        let state = result.continue_state;

        assert_eq!(state.resume_pc, 0);
        assert_eq!(state.resume_output_ind, 0);
        assert_eq!(state.program_state.tape_head, 0);
        // Ensure the tape is initialized to all zeros.
        assert!(state.program_state.tape.iter().all(|&x| x == 0));
    }

    /// Verifies that the generated jump table is the correct size
    #[test]
    fn test_jump_table_size() {
        let input = "+[]";
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_ok());
        let info = result.unwrap();

        // The jump table should have the same number of entries as the code size.
        assert_eq!(info.jump_table.len(), info.code.size());
    }

    /// Verifies that only jump characters have valid jump targets and that all other jump table values are -1. There should be absolutly no -2 values
    #[test]
    fn test_jump_table_values() {
        let input = ">++++++++[<+++++++++>-]<.>++++[<+++++++>-]<+.+++++++..+++.>>++++++[<+++++++>-]<++.------------.>++++++[<+++++++++>-]<+.<.+++.------.--------.>>>++++[<++++++++>-]<+.";
        let result = preprocess_input::<TAPE_SIZE>(input);
        assert!(result.is_ok());
        let info = result.unwrap();

        for (i, &value) in info.jump_table.iter().enumerate() {
            if info.code.get(i) == Some(BfInstruction::LoopStart) || info.code.get(i) == Some(BfInstruction::LoopEnd) {
                assert!(value != -2);
            } else {
                assert_eq!(value, -1);
            }
        }
    }
}