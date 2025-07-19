use crate::data::{BfInstruction, CompressedBF};
use std::any::Any;
use ahash::{HashMap, HashMapExt, HashSet, HashSetExt};
use std::sync::OnceLock;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use lazy_static::lazy_static;


#[derive(Debug, Eq, PartialEq)]
pub enum BfRunResult<const MAX_TAPE_SIZE: usize> {
    NOOPError,
    TargetMismatchError,
    TapeHeadBoundError,
    OOMError,
    InfiniteLoopError,
    InputTokenError,
    IncompleteLoopSuccess(ContinueState<MAX_TAPE_SIZE>),
    IncompleteOutputSuccess(ContinueState<MAX_TAPE_SIZE>),
    Success,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContinueState<const MAX_TAPE_SIZE: usize> {
    pub(crate) program_state: ProgramState<MAX_TAPE_SIZE>,
    pub(crate) resume_pc: usize,
    pub(crate) resume_output_ind: usize,
}

#[derive(Debug)]
pub struct RunningProgramInfo<const MAX_TAPE_SIZE: usize> {
    pub(crate) code: CompressedBF,
    pub(crate) current_paren_count: usize,
    pub(crate) jump_table: Vec<i64>,
    pub(crate) continue_state: ContinueState<MAX_TAPE_SIZE>,
}

//make sure to keep same vector capacity for Vec in order to save a lot of time on memory operations
impl<const MAX_TAPE_SIZE: usize> Clone for RunningProgramInfo<MAX_TAPE_SIZE> {
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
pub struct ProgramState<const MAX_TAPE_SIZE: usize> {
    pub(crate) tape: [u8; MAX_TAPE_SIZE],
    pub(crate) tape_head: u8,
}

pub static HASHSET_SIZE_HISTOGRAM: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();


lazy_static! {
    static ref GLOBAL: Mutex<HashMap<usize, Box<dyn Any + Send + Sync>>> =
        Mutex::new(HashMap::new());
}


fn get_state_tracker<const MAX_TAPE_SIZE: usize>() -> Arc<Mutex<Vec<HashSet<ProgramState<MAX_TAPE_SIZE>>>>> {
    let mut global = GLOBAL.lock().unwrap();
    let entry = global.entry(MAX_TAPE_SIZE).or_insert_with(|| {
        Box::new(HashMap::<ThreadId, Arc<Vec<HashSet<ProgramState<MAX_TAPE_SIZE>>>>>::new())
            as Box<dyn Any + Send + Sync>
    });

    // Downcast to the correct type
    let map = entry.downcast_mut::<HashMap<ThreadId, Arc<Mutex<Vec<HashSet<ProgramState<MAX_TAPE_SIZE>>>>>>>().unwrap();

    let thread_id = std::thread::current().id();
    map.entry(thread_id)
        .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

const SHRINK_TO_SIZE: usize = 2147483649;

pub fn run_program_fragment<const MAX_TAPE_SIZE: usize>(
    program_fragment: &RunningProgramInfo<MAX_TAPE_SIZE>,
    target_output: &[u8],
) -> BfRunResult<MAX_TAPE_SIZE> {
    let state_tracker_arc_mutex = get_state_tracker::<MAX_TAPE_SIZE>();
    let mut state_tracker = state_tracker_arc_mutex.lock().unwrap();
    {
        //clear the state tracker for this thread
        for state in state_tracker.iter_mut() {
            state.clear();
            state.shrink_to(SHRINK_TO_SIZE);
        }
        // Ensure the state tracker has enough elements
        if state_tracker.len() < program_fragment.code.size() {
            state_tracker.resize_with(program_fragment.code.size(), || {
                HashSet::with_capacity(256 * 4)
            });
        }

        let mut tape = program_fragment.continue_state.program_state.tape;
        let mut tape_head = program_fragment.continue_state.program_state.tape_head;
        let mut pc = program_fragment.continue_state.resume_pc; // Start from the last instruction
        let mut output_ind = program_fragment.continue_state.resume_output_ind; // Resume from the last output index

        while pc < program_fragment.code.size() {
            let current_state = ProgramState {
                tape: tape.clone(),
                tape_head,
            };
            if state_tracker[pc].contains(&current_state) {
                return collect_and_return(BfRunResult::InfiniteLoopError, &state_tracker);
            } else {
                state_tracker[pc].insert(current_state);
            }

            match program_fragment.code.get(pc) {
                None => {
                    panic!("could not read current BF instruction, pc: {}, program: {:?}", pc, program_fragment.code);
                }
                Some(BfInstruction::Inc) => {
                    tape[tape_head as usize] = tape[tape_head as usize].wrapping_add(1);
                }
                Some(BfInstruction::Dec) => {
                    tape[tape_head as usize] = tape[tape_head as usize].wrapping_sub(1);
                }
                Some(BfInstruction::Left) => {
                    if tape_head == 0 {
                        return collect_and_return(BfRunResult::TapeHeadBoundError, &state_tracker);
                    }
                    tape_head -= 1;
                }
                Some(BfInstruction::Right) => {
                    if tape_head as usize + 1 == MAX_TAPE_SIZE {
                        return collect_and_return(BfRunResult::OOMError, &state_tracker);
                    }
                    tape_head += 1;
                }
                Some(BfInstruction::LoopStart) => {
                    if tape[tape_head as usize] == 0 {
                        if program_fragment.jump_table[pc] == -1 {
                            panic!("jump table is not initialized correctly, found -1 at LoopStart, pc: {}, program: {:?}, jump_table: {:?}", pc, program_fragment.code, program_fragment.jump_table);
                        }
                        if program_fragment.jump_table[pc] == -2 {
                            return collect_and_return(BfRunResult::NOOPError, &state_tracker);
                        }
                        pc = program_fragment.jump_table[pc] as usize;
                        continue;
                    }
                }
                Some(BfInstruction::LoopEnd) => {
                    if tape[tape_head as usize] != 0 {
                        if program_fragment.jump_table[pc] == -1 {
                            panic!("jump table is not initialized correctly, found -1 at LoopEnd, pc: {}, program: {:?}, jump_table: {:?}", pc, program_fragment.code, program_fragment.jump_table);
                        }
                        pc = program_fragment.jump_table[pc] as usize;
                        continue;
                    }
                }
                Some(BfInstruction::Output) => {
                    if output_ind == target_output.len() {
                        return collect_and_return(
                            BfRunResult::TargetMismatchError,
                            &state_tracker,
                        );
                    }
                    if target_output[output_ind] != tape[tape_head as usize] {
                        return collect_and_return(
                            BfRunResult::TargetMismatchError,
                            &state_tracker,
                        );
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
}

const MAX_STEPS: usize = 131066;

static MAX_STEPS_REACHED: AtomicUsize = AtomicUsize::new(0);

pub fn run_program_fragment_without_states<const MAX_TAPE_SIZE: usize>(
    program_fragment: &RunningProgramInfo<MAX_TAPE_SIZE>,
    target_output: &[u8],
) -> BfRunResult<MAX_TAPE_SIZE> {
    let mut steps = 0;

    let mut tape = program_fragment.continue_state.program_state.tape;
    let mut tape_head = program_fragment.continue_state.program_state.tape_head;
    let mut pc = program_fragment.continue_state.resume_pc; // Start from the last instruction
    let mut output_ind = program_fragment.continue_state.resume_output_ind; // Resume from the last output index

    while pc < program_fragment.code.size() {
        steps += 1;

        if steps > MAX_STEPS {
            // Do not update MAX_STEPS_REACHED here, as this is the fallback to run_program_fragment
            return run_program_fragment(program_fragment, target_output);
        }

        match program_fragment.code.get(pc) {
            None => {
                MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                panic!(
                    "could not read current BF instruction, pc: {}, program: {:?}",
                    pc, program_fragment.code
                );
            }
            Some(BfInstruction::Inc) => {
                tape[tape_head as usize] = tape[tape_head as usize].wrapping_add(1);
            }
            Some(BfInstruction::Dec) => {
                tape[tape_head as usize] = tape[tape_head as usize].wrapping_sub(1);
            }
            Some(BfInstruction::Left) => {
                if tape_head == 0 {
                    MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                    return BfRunResult::TapeHeadBoundError;
                }
                tape_head -= 1;
            }
            Some(BfInstruction::Right) => {
                if tape_head as usize + 1 == MAX_TAPE_SIZE {
                    MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                    return BfRunResult::OOMError;
                }
                tape_head += 1;
            }
            Some(BfInstruction::LoopStart) => {
                if tape[tape_head as usize] == 0 {
                    if program_fragment.jump_table[pc] == -1 {
                        MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                        panic!(
                            "jump table is not initialized correctly, found -1 at LoopStart, pc: {}, program: {:?}, jump_table: {:?}",
                            pc, program_fragment.code, program_fragment.jump_table
                        );
                    }
                    if program_fragment.jump_table[pc] == -2 {
                        MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                        return BfRunResult::NOOPError;
                    }
                    pc = program_fragment.jump_table[pc] as usize;
                    continue;
                }
            }
            Some(BfInstruction::LoopEnd) => {
                if tape[tape_head as usize] != 0 {
                    if program_fragment.jump_table[pc] == -1 {
                        MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                        panic!(
                            "jump table is not initialized correctly, found -1 at LoopEnd, pc: {}, program: {:?}, jump_table: {:?}",
                            pc, program_fragment.code, program_fragment.jump_table
                        );
                    }
                    pc = program_fragment.jump_table[pc] as usize;
                    continue;
                }
            }
            Some(BfInstruction::Output) => {
                if output_ind == target_output.len() {
                    MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                    return BfRunResult::TargetMismatchError;
                }
                if target_output[output_ind] != tape[tape_head as usize] {
                    MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                    return BfRunResult::TargetMismatchError;
                }
                output_ind += 1;
            }
            Some(BfInstruction::Input) => {
                MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
                return BfRunResult::InputTokenError;
            }
        }
        pc += 1;
    }

    if program_fragment.current_paren_count != 0 {
        MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
        return BfRunResult::IncompleteLoopSuccess(ContinueState {
            program_state: ProgramState {
                tape: tape.clone(),
                tape_head,
            },
            resume_pc: pc,
            resume_output_ind: output_ind,
        });
    }

    let result = if output_ind != target_output.len() {
        BfRunResult::IncompleteOutputSuccess(ContinueState {
            program_state: ProgramState {
                tape: tape.clone(),
                tape_head,
            },
            resume_pc: pc,
            resume_output_ind: output_ind,
        })
    } else {
        BfRunResult::Success
    };
    MAX_STEPS_REACHED.fetch_max(steps, std::sync::atomic::Ordering::Relaxed);
    result
}

pub fn get_max_steps_reached() -> usize {
    MAX_STEPS_REACHED.load(std::sync::atomic::Ordering::Relaxed)
}

// fn tabulate_hashset_sizes<(state_tracker: &[HashSet<ProgramState>]) {
//     if let Some(hist) = HASHSET_SIZE_HISTOGRAM.get() {
//         let mut map = hist.lock().unwrap();
//         for size in state_tracker.iter().map(|s| s.len()) {
//             *map.entry(size).or_insert(0) += 1;
//         }
//     }
// }

fn collect_and_return<const MAX_TAPE_SIZE: usize>(
    result: BfRunResult<MAX_TAPE_SIZE>,
    state_tracker: &[HashSet<ProgramState<MAX_TAPE_SIZE>>],
) -> BfRunResult<MAX_TAPE_SIZE> {
    // tabulate_hashset_sizes(state_tracker);
    result
}
