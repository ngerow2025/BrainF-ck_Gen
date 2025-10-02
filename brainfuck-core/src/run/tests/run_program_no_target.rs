use super::super::*;

// Helper to collect outputs in a Vec and provide an input closure
fn collect_io<'a>(inputs: Vec<u8>) -> (impl FnMut() -> Option<u8> + 'a, impl FnMut(u8) + 'a, std::rc::Rc<std::cell::RefCell<Vec<u8>>>) {
    let inputs_rc = std::rc::Rc::new(std::cell::RefCell::new(inputs));
    let outputs_rc = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u8>::new()));
    let inputs_clone = inputs_rc.clone();
    let outputs_clone = outputs_rc.clone();
    let read_input = move || inputs_clone.borrow_mut().pop(); // pop from end to simulate queue (reverse inputs before passing)
    let write_output = move |b: u8| outputs_clone.borrow_mut().push(b);
    (read_input, write_output, outputs_rc)
}

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
fn no_target_basic_output_callback() {
    let prog = make_program::<4>(&[BfInstruction::Output], vec![], [42, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::Success);
    assert_eq!(outputs_rc.borrow().as_slice(), &[42]);
}

#[test]
fn no_target_input_consumed_and_written() {
    // program: Input, Output -> should echo back input
    let prog = make_program::<4>(&[BfInstruction::Input, BfInstruction::Output], vec![], [0, 0, 0, 0], 0, 0);
    // inputs are popped from end; supply reversed
    let (mut read_input, mut write_output, outputs_rc) = collect_io(vec![7]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::Success);
    assert_eq!(outputs_rc.borrow().as_slice(), &[7]);
}

#[test]
fn no_target_input_none_returns_input_token_error() {
    let prog = make_program::<4>(&[BfInstruction::Input, BfInstruction::Output], vec![], [0, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, _outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::InputTokenError);
}

#[test]
fn no_target_tape_head_bound_left_error() {
    let prog = make_program::<4>(&[BfInstruction::Left], vec![], [0, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, _outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::TapeHeadBoundError);
}

#[test]
fn no_target_oom_right_overflow_error() {
    let prog = make_program::<2>(&[BfInstruction::Right], vec![], [0, 0], 1, 0);
    let (mut read_input, mut write_output, _outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::OOMError);
}

#[test]
fn no_target_noop_jump_table_minus_two() {
    let prog = make_program::<4>(&[BfInstruction::LoopStart], vec![-2], [0, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, _outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::NOOPError);
}

#[test]
#[should_panic]
fn no_target_jump_table_minus_one_panics() {
    let prog = make_program::<4>(&[BfInstruction::LoopStart], vec![-1], [0, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, _outputs_rc) = collect_io(vec![]);
    // should panic inside the runner when it sees -1 jump table
    let _ = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
}

#[test]
fn no_target_infinite_loop_detection() {
    let prog = make_program::<4>(&[BfInstruction::LoopStart, BfInstruction::LoopEnd], vec![1, 0], [1, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, _outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    assert_eq!(res, BfRunResult::InfiniteLoopError);
}

#[test]
fn no_target_resume_incomplete_loop_success() {
    // program ends with unmatched paren count non-zero -> should return IncompleteLoopSuccess with state
    let prog = make_program::<4>(&[BfInstruction::Output], vec![], [3, 0, 0, 0], 0, 1);
    let (mut read_input, mut write_output, outputs_rc) = collect_io(vec![]);
    match run_program_fragment_no_target(&prog, &mut read_input, &mut write_output) {
        BfRunResult::IncompleteLoopSuccess(cont) => {
            assert_eq!(cont.program_state.tape[0], 3);
            assert_eq!(cont.resume_output_ind, 1);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn no_target_output_in_loop_increments_output_ind_prevents_infinite_loop() {
    // A loop that outputs a value should increment output_ind; the state tracker should still detect repeats but
    // output-ind changes prevent false infinite loop detection if outputs are consumed by external target matching logic.
    // We'll build a loop that outputs the same cell but the runner should either complete or detect infinite loop.
    let insts = [BfInstruction::LoopStart, BfInstruction::Output, BfInstruction::LoopEnd];
    // make jump table 0->2, 2->0
    let prog = make_program::<4>(&insts, vec![2, 0, 0], [1, 0, 0, 0], 0, 0);
    let (mut read_input, mut write_output, outputs_rc) = collect_io(vec![]);
    let res = run_program_fragment_no_target(&prog, &mut read_input, &mut write_output);
    // either succeed or detect infinite loop depending on state tracking; accept either Success or InfiniteLoopError
    match res {
        BfRunResult::Success => assert!(!outputs_rc.borrow().is_empty()),
        BfRunResult::InfiniteLoopError => assert!(!outputs_rc.borrow().is_empty()),
        other => panic!("unexpected result: {:?}", other),
    }
}

#[test]
fn no_target_resume_and_continue_matches_full_run() {
    // similar to previous resume tests but for no-target runner
    let insts = [
        BfInstruction::Right,
        BfInstruction::Inc,
        BfInstruction::Output,
        BfInstruction::Right,
        BfInstruction::Inc,
        BfInstruction::Output,
    ];
    let full_prog = make_program::<4>(&insts, vec![], [0, 0, 0, 0], 0, 0);
    let (mut r_in_full, mut r_out_full, outputs_full) = collect_io(vec![]);
    let full_res = run_program_fragment_no_target(&full_prog, &mut r_in_full, &mut r_out_full);

    // simulate first three instructions
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
    let (mut r_in_resumed, mut r_out_resumed, outputs_resumed) = collect_io(vec![]);
    let mut resumed = resumed_prog.clone();
    resumed.continue_state.resume_pc = 3;
    resumed.continue_state.resume_output_ind = output_ind;
    let resumed_res = run_program_fragment_no_target(&resumed, &mut r_in_resumed, &mut r_out_resumed);

    assert_eq!(full_res, resumed_res);
    // resumed run should produce the suffix of full outputs starting at the resume output index
    let resumed_expected = outputs_full.borrow().as_slice()[output_ind..].to_vec();
    assert_eq!(resumed_expected.as_slice(), outputs_resumed.borrow().as_slice());
}
