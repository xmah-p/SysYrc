use lalrpop_util::lalrpop_mod;
use std::env::args;
use std::fs::read_to_string;
use std::io::Result;

pub mod ast;
pub mod front_end;

lalrpop_mod!(sysy);  

// cmdline example: sysyrc <mode> <input> -o <output>
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
    // 假设 parse_cmdline 返回 (String, String, String)
    let (mode, input, output) = parse_cmdline();

    let input: String = read_to_string(input)?;

    let parser = sysy::CompUnitParser::new();

    let Ok(ast) = parser.parse(&input) else {
        panic!("Failed to parse input"); 
    };

    let Ok(koopa_ir) = front_end::translate_to_koopa(ast) else {
        panic!("Failed to translate to Koopa IR");
    };

    match mode.as_str() {
        "-koopa" => {
            front_end::emit_lr(&koopa_ir, std::fs::File::create(output)?)?;
        }
        "-riscv" => {
            
        }
        "-perf" => {
            panic!("Perf backend not implemented yet");
        }
        _ => panic!("Unknown mode: {}", mode),
    };

    Ok(())
}