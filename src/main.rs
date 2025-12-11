use lalrpop_util::lalrpop_mod;
use std::env::args;
use std::fs::read_to_string;
use std::io::Result;

pub mod ast;
pub mod frontend;
pub mod backend;

lalrpop_mod!(sysy);  

// Cmdline example: sysyrc <mode> <input> -o <output>
// No error handling yet for simplicity
fn parse_cmdline() -> (String, String, String) {
    let mut args = args();
    args.next();
    let mode = args.next().unwrap();
    let input = args.next().unwrap();
    args.next();
    let output = args.next().unwrap();
    (mode, input, output)
}

fn main() -> Result<()> {
    let (mode, input, output) = parse_cmdline();

    let output = std::fs::File::create(output)?;
    let writer = std::io::BufWriter::new(output);

    let input: String = read_to_string(input)?;

    let parser = sysy::CompUnitParser::new();

    let Ok(ast) = parser.parse(&input) else {
        panic!("Failed to parse input"); 
    };

    let koopa_ir = frontend::translate_to_koopa(ast);


    match mode.as_str() {
        "-koopa" => {
            frontend::emit_ir(&koopa_ir, writer)?;
        }
        "-riscv" => {
            backend::emit_riscv(&koopa_ir, writer)?;
        }
        "-perf" => {
            panic!("Perf backend not implemented yet");
        }
        _ => panic!("Unknown mode: {}", mode),
    };

    Ok(())
}