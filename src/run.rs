use crate::MAX_TAPE_SIZE;
use crate::data::{BfInstruction, CompressedBF};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::OnceLock;
use ahash::RandomState;

#[derive(Debug, Eq, PartialEq)]
pub enum BfRunResult {
    NOOPError,
    TargetMismatchError,
    TapeHeadBoundError,
    OOMError,
    InfiniteLoopError,
    InputTokenError,
    IncompleteLoopSuccess(ContinueState),
    IncompleteOutputSuccess(ContinueState),
    Success,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContinueState {
    pub(crate) program_state: ProgramState,
    pub(crate) resume_pc: usize,
    pub(crate) resume_output_ind: usize,
}

#[derive(Debug)]
pub struct RunningProgramInfo {
    pub(crate) code: CompressedBF,
    pub(crate) current_paren_count: usize,
    pub(crate) jump_table: Vec<i64>,
    pub(crate) continue_state: ContinueState,
}

//make sure to keep same vector capacity for Vec in order to save a lot of time on memory operations
impl Clone for RunningProgramInfo {
    fn clone(&self) -> Self {
        let mut new_jump_table = Vec::with_capacity(self.jump_table.capacity());
        new_jump_table.extend_from_slice(&self.jump_table);
        RunningProgramInfo {
            code: self.code.clone(),
            current_paren_count: self.current_paren_count,
            jump_table: new_jump_table,
            continue_state: self.continue_state.clone(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProgramState {
    pub(crate) tape: [u8; MAX_TAPE_SIZE],
    pub(crate) tape_head: usize,
}

pub static HASHSET_SIZE_HISTOGRAM: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

pub fn run_program_fragment(
    program_fragment: &RunningProgramInfo,
    target_output: &[u8],
) -> BfRunResult {
    let mut state_tracker = vec![HashSet::with_capacity_and_hasher(256, RandomState::new()); program_fragment.code.size()];

    let mut tape = program_fragment.continue_state.program_state.tape;
    let mut tape_head = program_fragment.continue_state.program_state.tape_head;
    let mut pc = program_fragment.continue_state.resume_pc; // Start from the last instruction
    let mut output_ind = program_fragment.continue_state.resume_output_ind; // Resume from the last output index

    while pc < program_fragment.code.size() {
        let current_state = ProgramState {
            tape: tape.clone(),
            tape_head,
        };
        if (state_tracker[pc].contains(&current_state)) {
            return collect_and_return(
                BfRunResult::InfiniteLoopError,
                &state_tracker,
            );
        } else {
            state_tracker[pc].insert(current_state);
        }

        match program_fragment.code.get(pc) {
            None => {
                panic!("could not read current BF instruction");
            }
            Some(BfInstruction::Inc) => {
                tape[tape_head] = tape[tape_head].wrapping_add(1);
            }
            Some(BfInstruction::Dec) => {
                tape[tape_head] = tape[tape_head].wrapping_sub(1);
            }
            Some(BfInstruction::Left) => {
                if tape_head == 0 {
                    return collect_and_return(                        
                        BfRunResult::TapeHeadBoundError,
                        &state_tracker,
                    );
                }
                tape_head -= 1;
            }
            Some(BfInstruction::Right) => {
                if tape_head + 1 == MAX_TAPE_SIZE {
                    return collect_and_return(
                        BfRunResult::TapeHeadBoundError,
                        &state_tracker,
                    );
                }
                tape_head += 1;
            }
            Some(BfInstruction::LoopStart) => {
                if tape[tape_head] == 0 {
                    if program_fragment.jump_table[pc] == -1 {
                        panic!("jump table is not initialized correctly");
                    }
                    if program_fragment.jump_table[pc] == -2 {
                        return collect_and_return(BfRunResult::NOOPError, &state_tracker);
                    }
                    pc = program_fragment.jump_table[pc] as usize;
                    continue;
                }
            }
            Some(BfInstruction::LoopEnd) => {
                if tape[tape_head] != 0 {
                    if program_fragment.jump_table[pc] == -1 {
                        panic!("jump table is not initialized correctly");
                    }
                    pc = program_fragment.jump_table[pc] as usize;
                    continue;
                }
            }
            Some(BfInstruction::Output) => {
                if output_ind == target_output.len() {
                    return collect_and_return(BfRunResult::TargetMismatchError, &state_tracker);
                }
                if target_output[output_ind] != tape[tape_head] {
                    return collect_and_return(BfRunResult::TargetMismatchError, &state_tracker);
                }
                output_ind += 1;
            }
            Some(BfInstruction::Input) => {
                return collect_and_return(BfRunResult::InputTokenError, &state_tracker);
            }
        }
        pc += 1;
    }

    if program_fragment.current_paren_count != 0 {
        return collect_and_return(
            BfRunResult::IncompleteLoopSuccess(ContinueState {
                program_state: ProgramState {
                    tape: tape.clone(),
                    tape_head,
                },
                resume_pc: pc,
                resume_output_ind: output_ind,
            }),
            &state_tracker,
        );
    }

    if output_ind != target_output.len() {
        collect_and_return(
            BfRunResult::IncompleteOutputSuccess(ContinueState {
                program_state: ProgramState {
                    tape: tape.clone(),
                    tape_head,
                },
                resume_pc: pc,
                resume_output_ind: output_ind,
            }),
            &state_tracker,
        )
    } else {
        collect_and_return(BfRunResult::Success, &state_tracker)
    }
}

fn tabulate_hashset_sizes(state_tracker: &[HashSet<ProgramState, RandomState>]) {
    if let Some(hist) = HASHSET_SIZE_HISTOGRAM.get() {
        let mut map = hist.lock().unwrap();
        for size in state_tracker.iter().map(|s| s.len()) {
            *map.entry(size).or_insert(0) += 1;
        }
    }
}

fn collect_and_return(result: BfRunResult, state_tracker: &[HashSet<ProgramState, RandomState>]) -> BfRunResult {
    // tabulate_hashset_sizes(state_tracker);
    result
}
