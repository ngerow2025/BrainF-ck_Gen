use super::super::*;

fn make_program<const N: usize>(
    insts: &[BfInstruction],
    jump_table: Vec<i64>,
    tape: [u8; N],
    tape_head: u8,
    paren_count: usize,
) -> RunningProgramInfo<N> {
    let mut code = CompressedBF::new(0, 0);
    for &i in insts {
        code.append(i);
    }
    RunningProgramInfo {
        code,
        current_paren_count: paren_count,
        jump_table,
        continue_state: ContinueState {
            program_state: ProgramState { tape, tape_head },
            resume_pc: 0,
            resume_output_ind: 0,
        },
    }
}

#[test]
fn success_when_output_matches() {
    let prog = make_program::<4>(&[BfInstruction::Output], vec![], [7, 0, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[7]);
    assert_eq!(res, BfRunResult::Success);
}

#[test]
fn target_mismatch_wrong_value() {
    let prog = make_program::<4>(&[BfInstruction::Output], vec![], [7, 0, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[6]);
    assert_eq!(res, BfRunResult::TargetMismatchError);
}

#[test]
fn target_mismatch_short_target_when_more_outputs() {
    // program produces two outputs but target only expects one -> TargetMismatchError
    let prog = make_program::<4>(&[BfInstruction::Output, BfInstruction::Output], vec![], [1, 2, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[1]);
    assert_eq!(res, BfRunResult::TargetMismatchError);
}

#[test]
fn incomplete_output_on_end() {
    // program outputs 1 value but target expects 2 -> IncompleteOutputSuccess with resume_output_ind == 1
    let prog = make_program::<4>(&[BfInstruction::Output], vec![], [9, 0, 0, 0], 0, 0);
    match run_program_fragment(&prog, &[9, 10]) {
        BfRunResult::IncompleteOutputSuccess(cont) => {
            assert_eq!(cont.resume_output_ind, 1);
        }
        other => panic!("unexpected result: {:?}", other),
    }
}

#[test]
fn input_token_error_in_target_runner() {
    let prog = make_program::<4>(&[BfInstruction::Input], vec![], [0, 0, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[]);
    assert_eq!(res, BfRunResult::InputTokenError);
}

#[test]
fn tape_head_bound_left_error() {
    let prog = make_program::<4>(&[BfInstruction::Left], vec![], [0, 0, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[]);
    assert_eq!(res, BfRunResult::TapeHeadBoundError);
}

#[test]
fn oom_right_overflow_error() {
    // use small tape to trigger OOM when moving right from last cell
    let prog = make_program::<2>(&[BfInstruction::Right], vec![], [0, 0], 1, 0);
    let res = run_program_fragment(&prog, &[]);
    assert_eq!(res, BfRunResult::OOMError);
}

#[test]
fn noop_jump_table_minus_two_returns_noop() {
    // LoopStart with jump_table == -2 should return NOOPError when cell == 0
    let prog = make_program::<4>(&[BfInstruction::LoopStart], vec![-2], [0, 0, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[]);
    assert_eq!(res, BfRunResult::NOOPError);
}

#[test]
fn jump_table_minus_one_panics() {
    use std::panic::catch_unwind;
    // LoopStart with jump_table == -1 triggers panic path
    let prog = make_program::<4>(&[BfInstruction::LoopStart], vec![-1], [0, 0, 0, 0], 0, 0);
    let panicked = catch_unwind(|| {
        let _ = run_program_fragment(&prog, &[]);
    })
    .is_err();
    assert!(panicked);
}

#[test]
fn infinite_loop_detection() {
    // empty loop: [ ] with initial cell != 0 will loop forever without changing state
    // jump_table maps 0 -> 1 and 1 -> 0
    let prog = make_program::<4>(&[BfInstruction::LoopStart, BfInstruction::LoopEnd], vec![1, 0], [1, 0, 0, 0], 0, 0);
    let res = run_program_fragment(&prog, &[]);
    assert_eq!(res, BfRunResult::InfiniteLoopError);
}

#[test]
fn resume_partial_run_matches_full_run() {
    // program: > + . > + .  (two outputs from two different cells)
    let insts = [
        BfInstruction::Right,
        BfInstruction::Inc,
        BfInstruction::Output,
        BfInstruction::Right,
        BfInstruction::Inc,
        BfInstruction::Output,
    ];
    // full program
    let full_prog = make_program::<4>(&insts, vec![], [0, 0, 0, 0], 0, 0);
    let full_res = run_program_fragment(&full_prog, &[1, 1]);
    assert_eq!(full_res, BfRunResult::Success);

    // simulate first three instructions manually to create a resume state
    let mut tape = [0u8; 4];
    let mut tape_head: u8 = 0;
    let mut output_ind = 0usize;
    for &i in &insts[..3] {
        match i {
            BfInstruction::Inc => tape[tape_head as usize] = tape[tape_head as usize].wrapping_add(1),
            BfInstruction::Dec => tape[tape_head as usize] = tape[tape_head as usize].wrapping_sub(1),
            BfInstruction::Left => tape_head -= 1,
            BfInstruction::Right => tape_head += 1,
            BfInstruction::Output => output_ind += 1,
            BfInstruction::Input => {},
            _ => {},
        }
    }

    let resumed_prog = make_program::<4>(&insts, vec![], tape, tape_head, 0);
    // set resume pc and output index to reflect partial progress
    let mut resumed = resumed_prog.clone();
    resumed.continue_state.resume_pc = 3; // after first three instructions
    resumed.continue_state.resume_output_ind = output_ind;

    let resumed_res = run_program_fragment(&resumed, &[1, 1]);
    assert_eq!(resumed_res, BfRunResult::Success);
}

#[test]
fn resume_with_nonzero_output_index_behaves_same_as_full() {
    // program: . . . (three outputs). We'll simulate first two outputs already matched.
    let insts = [BfInstruction::Output, BfInstruction::Output, BfInstruction::Output];
    let tape = [5u8, 6u8, 7u8, 0u8];
    let full_prog = make_program::<4>(&insts, vec![], tape, 0, 0);
    // full run against [5,6,7]
    let full_res = run_program_fragment(&full_prog, &[5, 6, 7]);

    // create a resume state that has already matched first two outputs
    let mut resumed = make_program::<4>(&insts, vec![], tape, 0, 0);
    resumed.continue_state.resume_pc = 2; // start at 3rd output
    resumed.continue_state.resume_output_ind = 2; // already matched two outputs

    // resume run with full target still [5,6,7]
    let resumed_res = run_program_fragment(&resumed, &[5, 6, 7]);
    assert_eq!(resumed_res, full_res);
}
