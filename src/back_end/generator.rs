use super::context::RiscvContext;
use koopa::ir::entities::ValueData;
use koopa::ir::{FunctionData, Program, ValueKind};
use std::fmt;

pub trait GenerateRiscv {
    // fmt::Result is an alias for Result<(), fmt::Error>
    // Lifetime parameter 'a ensures that the context
    // lives at least as long as the data being generated
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result;
}

impl GenerateRiscv for Program {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        context.write_inst(".text")?;
        context.write_inst(".globl main")?;

        for &func in self.func_layout() {
            let func_data: &FunctionData = self.func(func);
            context.current_func = Some(func_data);
            func_data.generate(context)?;
            context.current_func = None;
        }
        Ok(())
    }
}

impl GenerateRiscv for FunctionData {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        // Function name starts with an '@'
        let name = self.name().replace("@", "");
        context.write_line(&format!("{}:", name))?;

        for (&bb, node) in self.layout().bbs() {
            // node is a BasicBlockNode
            let bb_name = self.dfg().bb(bb).name().as_ref().unwrap().replace("%", "");
            context.write_line(&format!("{}:", bb_name))?;

            for &inst in node.insts().keys() {
                // inst is a Value
                let inst_data = self.dfg().value(inst);
                inst_data.generate(context)?;
            }
        }
        Ok(())
    }
}

impl GenerateRiscv for ValueData {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        match self.kind() {
            ValueKind::Integer(_) => {}

            ValueKind::Return(value) => {
                let Some(ret_value) = value.value() else {
                    // error
                    panic!("Unsupported return instruction without value");
                };
                let ret_value_data = context.current_func.unwrap().dfg().value(ret_value);
                match ret_value_data.kind() {
                    ValueKind::Integer(int) => {
                        context.write_inst(&format!("li a0, {}", int.value()))?;
                    }
                    _ => {
                        panic!("Unsupported return value type in RISC-V generation");
                    }
                }
                context.write_inst("ret")?;
            }
            _ => {
                panic!("Unsupported instruction in RISC-V generation");
            }
        }
        Ok(())
    }
}
