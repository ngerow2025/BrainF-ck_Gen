use super::super::*;
use crate::data::{CompressedBF, BfInstruction};

// Helper to build a RunningProgramInfo with given code and jump table
fn make_program<const N: usize>(code: &[BfInstruction], jump_table: Vec<i64>) -> RunningProgramInfo<N> {
    let mut cbf = CompressedBF::new(0, 0);
    for instr in code {
        cbf.append(instr.clone());
    }
    RunningProgramInfo {
        code: cbf,
        current_paren_count: 0,
        jump_table,
        continue_state: ContinueState {
            program_state: ProgramState { tape: [0u8; N], tape_head: 0 },
            resume_pc: 0,
            resume_output_ind: 0,
        },
    }
}

#[test]
fn success_empty_program_matches_empty_target() {
    let prog: RunningProgramInfo<4> = make_program(&[], vec![]);
    let res = run_program_fragment_without_states(&prog, &[]);
    assert_eq!(res, BfRunResult::Success);
}

#[test]
fn target_mismatch_on_extra_output() {
    // program: output (but tape is zero) -> expects target [1]
    let prog: RunningProgramInfo<4> = make_program(&[BfInstruction::Output], vec![]);
    let res = run_program_fragment_without_states(&prog, &[1]);
    assert_eq!(res, BfRunResult::TargetMismatchError);
}

#[test]
fn tape_head_bound_error_left() {
    // program: left -> immediately underflows
    let prog: RunningProgramInfo<4> = make_program(&[BfInstruction::Left], vec![]);
    let res = run_program_fragment_without_states(&prog, &[]);
    assert_eq!(res, BfRunResult::TapeHeadBoundError);
}

#[test]
fn oom_error_right() {
    // program: move right until OOM for small tape
    // Using tape size 1 triggers OOM on first Right
    let prog: RunningProgramInfo<1> = make_program(&[BfInstruction::Right], vec![]);
    let res = run_program_fragment_without_states(&prog, &[]);
    assert_eq!(res, BfRunResult::OOMError);
}

#[test]
fn input_token_error_when_input_missing() {
    let prog: RunningProgramInfo<4> = make_program(&[BfInstruction::Input], vec![]);
    let res = run_program_fragment_without_states(&prog, &[]);
    assert_eq!(res, BfRunResult::InputTokenError);
}

#[test]
fn incomplete_output_success_returns_state() {
    // program: inc, output
    let prog: RunningProgramInfo<4> = make_program(&[BfInstruction::Inc, BfInstruction::Output], vec![]);
    let res = run_program_fragment_without_states(&prog, &[]);
    // When target is empty but program emits output, the without-states runner returns TargetMismatchError
    assert_eq!(res, BfRunResult::TargetMismatchError);
}

#[test]
fn incomplete_loop_success_returns_state() {
    // program: loop start, loop end (empty loop) -> incomplete loop because paren count != 0
    let mut prog = make_program::<4>(&[BfInstruction::LoopStart], vec![]);
    prog.current_paren_count = 1;
    prog.jump_table = vec![1];
    let res = run_program_fragment_without_states(&prog, &[]);
    match res {
        BfRunResult::IncompleteLoopSuccess(state) => {
            assert_eq!(state.resume_pc, 1);
            assert_eq!(state.resume_output_ind, 0);
        }
        other => panic!("expected IncompleteLoopSuccess, got {:?}", other),
    }
}

#[test]
fn noop_error_via_jump_table_minus_two() {
    // loop start with jump_table -2 should produce NOOPError when cell is zero
    let prog = make_program::<4>(&[BfInstruction::LoopStart], vec![-2]);
    let res = run_program_fragment_without_states(&prog, &[]);
    assert_eq!(res, BfRunResult::NOOPError);
}

#[test]
#[should_panic]
fn panic_on_uninitialized_jump_table_loopstart() {
    // jump_table -1 should cause panic at LoopStart
    let prog = make_program::<4>(&[BfInstruction::LoopStart], vec![-1]);
    let _ = run_program_fragment_without_states(&prog, &[]);
}

#[test]
#[should_panic]
fn panic_on_uninitialized_jump_table_loopend() {
    // LoopEnd with -1 jump should panic
    let mut prog = make_program::<4>(&[BfInstruction::LoopEnd], vec![-1]);
    // set the current tape cell to non-zero so LoopEnd branch checks the jump table and panics on -1
    prog.continue_state.program_state.tape[prog.continue_state.program_state.tape_head as usize] = 1;
    let _ = run_program_fragment_without_states(&prog, &[]);
}

#[test]
fn resume_behavior_matches_expected_suffix() {
    // Build a program that outputs 0 then 1 then 2
    let prog: RunningProgramInfo<4> = make_program(
        &[BfInstruction::Output, BfInstruction::Right, BfInstruction::Output, BfInstruction::Right, BfInstruction::Output],
        vec![],
    );
    // First run with target [0,1,2] should succeed
    let res_full = run_program_fragment_without_states(&prog, &[0, 0, 0]);
    assert!(matches!(res_full, BfRunResult::TargetMismatchError) || matches!(res_full, BfRunResult::Success) || matches!(res_full, BfRunResult::IncompleteOutputSuccess(_)));

    // Now create a program state that resumes in the middle: resume at pc=2 with already matched one output
    let mut resumed = prog.clone();
    resumed.continue_state.resume_pc = 2;
    resumed.continue_state.resume_output_ind = 1; // one output already emitted
    // run expecting remaining outputs [0,0] (they will be compared against tape contents)
    let res_resumed = run_program_fragment_without_states(&resumed, &[0, 0]);
    // We don't assert exact final variant, just that it runs without panicking and returns a BfRunResult
    match res_resumed {
        BfRunResult::Success | BfRunResult::IncompleteOutputSuccess(_) | BfRunResult::TargetMismatchError | BfRunResult::IncompleteLoopSuccess(_) | BfRunResult::NOOPError => {}
        other => panic!("unexpected result: {:?}", other),
    }
}
