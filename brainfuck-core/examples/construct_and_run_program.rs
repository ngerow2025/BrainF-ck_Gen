use brainfuck_core::{run_program_fragment_no_target, util::preprocess_input};



fn main(){
    const TAPE_SIZE: usize = 16;

    // This program writes 'A' (65) into the first cell by incrementing 65 times then outputs it.
    let program = ">+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++.";

    let info = preprocess_input::<TAPE_SIZE>(program).expect("preprocess failed");

    let mut out = Vec::new();
    let res = run_program_fragment_no_target(&info, || None, |b| out.push(b));

    println!("Result: {:?}", res);
    println!("Output bytes: {:?}", out);
    if !out.is_empty() {
        println!("As string: {}", out.iter().map(|b| *b as char).collect::<String>() );
    }
}
