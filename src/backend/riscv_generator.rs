use crate::backend::asm_writer::AsmWriter;
use crate::backend::stack_frame::{self, StackFrame};
use koopa::ir::entities::ValueData;
use koopa::ir::{values::BinaryOp as KoopaBinaryOp, *};
use std::cell::Ref;
use std::io::{self, Write};

pub const WORD_SIZE: i32 = 4;
const MAX_IMM_12: i32 = 2047; // Maximum positive immediate for 12-bit signed integer

pub struct RiscvGenerator<'a, W: Write> {
    program: &'a Program,
    writer: AsmWriter<W>,
}

impl<'a, W: Write> RiscvGenerator<'a, W> {
    pub fn new(program: &'a Program, writer: W) -> Self {
        Self {
            program,
            writer: AsmWriter::new(writer),
        }
    }

    pub fn generate_program(&mut self) -> io::Result<()> {
        let writer = &mut self.writer;
        let program = self.program;

        writer.write_directive("data", &[], false)?;
        for &global in program.inst_layout() {
            let global_data = program.borrow_value(global);
            let name = global_data.name().as_ref().unwrap().replace("@", "");
            writer.write_directive("globl", &[&name], true)?;
            writer.write_label(&name)?;
            match global_data.kind() {
                ValueKind::GlobalAlloc(alloc) => {
                    let init = alloc.init();
                    let init_data = program.borrow_value(init);
                    match init_data.kind() {
                        ValueKind::Integer(int) => {
                            writer.write_directive("word", &[&int.value().to_string()], true)?;
                        }
                        ValueKind::ZeroInit(_) => {
                            writer.write_directive("zero", &[&WORD_SIZE.to_string()], true)?;
                        }
                        _ => {
                            panic!("Unsupported global initializer");
                        }
                    }
                }
                _ => {
                    panic!("Unsupported global value kind");
                }
            }
        }

        for &func in program.func_layout() {
            let func_data = program.func(func);
            // Skip function declarations (none entry basic block)
            if func_data.layout().entry_bb().is_none() {
                continue;
            }
            self.generate_function(func_data)?;
        }
        Ok(())
    }

    fn generate_function(&mut self, func: &FunctionData) -> io::Result<()> {
        // Function name starts with an '@'
        let writer = &mut self.writer;

        let name = func.name().replace("@", "");
        writer.write_directive("text", &[], false)?;
        writer.write_directive("globl", &[&name], true)?;
        writer.write_label(&name)?;

        // Stack frame setup
        let mut stack_frame = StackFrame::new();
        stack_frame.initialize(func);
        self.generate_prologue(&stack_frame)?;
        self.save_caller_saved_regs(&stack_frame)?;

        // Generate code for each basic block
        for (&bb, node) in func.layout().bbs() {
            // node is a &BasicBlockNode
            let bb_name = Self::get_bb_name(func, bb);
            writer.write_label(&bb_name)?;

            // Generate code for each instruction in the basic block
            for &inst in node.insts().keys() {
                // inst is a Value
                let inst_data = func.dfg().value(inst);
                self.generate_instruction(inst_data, inst, &stack_frame, func)?;
            }
        }
        Ok(())
    }

    fn get_bb_name(func: &FunctionData, bb: BasicBlock) -> String {
        func.dfg().bb(bb).name().as_ref().unwrap().replace("@", "")
    }

    fn generate_prologue(&mut self, stack_frame: &StackFrame) -> io::Result<()> {
        let writer = &mut self.writer;
        let stack_size = stack_frame.get_stack_size();
        if stack_size == 0 {
            return Ok(());
        }
        let offset = (-stack_size).to_string();
        if stack_size > MAX_IMM_12 {
            writer.write_inst("li", &["t0", &offset])?;
            writer.write_inst("add", &["sp", "sp", "t0"])?;
        } else {
            writer.write_inst("addi", &["sp", "sp", &offset])?;
        }
        Ok(())
    }

    fn generate_epilogue(&mut self, stack_frame: &StackFrame) -> io::Result<()> {
        let writer = &mut self.writer;
        let stack_size = stack_frame.get_stack_size();
        if stack_size == 0 {
            return Ok(());
        }
        let offset = stack_size.to_string();
        if stack_size > MAX_IMM_12 {
            writer.write_inst("li", &["t0", &offset])?;
            writer.write_inst("add", &["sp", "sp", "t0"])?;
        } else {
            writer.write_inst("addi", &["sp", "sp", &offset])?;
        }
        Ok(())
    }

    fn prepare_addr(&mut self, offset: i32, tmp_reg: &str) -> io::Result<()> {
        let writer = &mut self.writer;
        if offset > MAX_IMM_12 || offset < -MAX_IMM_12 - 1 {
            writer.write_inst("li", &["t0", &offset.to_string()])?;
            writer.write_inst("add", &[tmp_reg, "sp", "t0"])?;
        }
        Ok(())
    }

    fn get_addr_str(&self, offset: i32, tmp_reg: &str) -> String {
        if offset <= MAX_IMM_12 && offset >= -MAX_IMM_12 - 1 {
            format!("{}(sp)", offset)
        } else {
            format!("0({})", tmp_reg)
        }
    }

    fn save_caller_saved_regs(&mut self, stack_frame: &StackFrame) -> io::Result<()> {
        let writer = &mut self.writer;
        let Some(ra_offset) = stack_frame.get_ra_offset() else {
            return Ok(());
        };
        self.prepare_addr(ra_offset, "t0")?;
        let addr = self.get_addr_str(ra_offset, "t0");
        writer.write_inst("sw", &["ra", &addr])
    }

    fn restore_caller_saved_regs(&mut self, stack_frame: &StackFrame) -> io::Result<()> {
        let writer = &mut self.writer;
        let Some(ra_offset) = stack_frame.get_ra_offset() else {
            return Ok(());
        };
        self.prepare_addr(ra_offset, "t0")?;
        let addr = self.get_addr_str(ra_offset, "t0");
        writer.write_inst("lw", &["ra", &addr])
    }

    fn load_global_value_to_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        let writer = &mut self.writer;
        let global_name = self
            .program
            .borrow_value(value)
            .name()
            .as_ref()
            .unwrap()
            .replace("@", "");
        writer.write_inst("la", &[reg, &global_name])?;
        writer.write_inst("lw", &[reg, &("0(".to_string() + reg + ")")])
    }

    fn load_local_value_to_reg(
        &mut self,
        stack_frame: &StackFrame,
        value_data: Ref<ValueData>,
        value: Value,
        reg: &str,
    ) -> io::Result<()> {
        let writer = &mut self.writer;
        match value_data.kind() {
            ValueKind::Integer(int) => {
                if int.value() == 0 {
                    writer.write_inst("mv", &[reg, "x0"])
                } else {
                    writer.write_inst("li", &[reg, &int.value().to_string()])
                }
            }
            ValueKind::FuncArgRef(arg) => {
                let arg_index = arg.index() as i32;
                if arg_index < 8 {
                    writer.write_inst("mv", &[reg, &format!("a{}", arg_index)])
                } else {
                    let offset = (arg_index - 8) * WORD_SIZE + stack_frame.get_stack_size();
                    self.prepare_addr(offset, reg)?;
                    let addr: String = self.get_addr_str(offset, reg);
                    writer.write_inst("lw", &[reg, &addr])
                }
            }
            // Result of other instructions
            // They should have been already stored on the stack
            _ => {
                let offset = stack_frame.get_stack_offset(value);
                self.prepare_addr(offset, "t0")?;
                let addr: String = self.get_addr_str(offset, "t0");
                writer.write_inst("lw", &[reg, &addr])
            }
        }
    }

    fn load_value_to_reg(&mut self, value: Value, reg: &str, stack_frame: &StackFrame) -> io::Result<()> {
        let value_data = self.program.borrow_value(value);
        if value.is_global() {
            self.load_global_value_to_reg(value, reg)
        } else {
            self.load_local_value_to_reg(stack_frame, value_data, value, reg)
        }
    }

    fn save_global_value_from_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        let writer = &mut self.writer;
        let global_name = self
            .program
            .borrow_value(value)
            .name()
            .as_ref()
            .unwrap()
            .replace("@", "");
        writer.write_inst("la", &["t0", &global_name])?;
        writer.write_inst("sw", &[reg, "0(t0)"])
    }

    fn save_local_value_from_reg(
        &mut self,
        stack_frame: &StackFrame,
        value_data: Ref<ValueData>,
        value: Value,
        reg: &str,
    ) -> io::Result<()> {
        let writer = &mut self.writer;
        let offset = stack_frame.get_stack_offset(value);
        self.prepare_addr(offset, "t0")?;
        let addr: String = self.get_addr_str(offset, "t0");
        writer.write_inst("sw", &[reg, &addr])
    }

    fn save_value_from_reg(&mut self, value: Value, reg: &str, stack_frame: &StackFrame) -> io::Result<()> {
        let value_data = self.program.borrow_value(value);
        if value.is_global() {
            self.save_global_value_from_reg(value, reg)
        } else {
            self.save_local_value_from_reg(stack_frame, value_data, value, reg)
        }
    }

    fn generate_instruction(
        &mut self,
        value_data: &ValueData,
        value: Value,
        stack_frame: &StackFrame,
        func: &FunctionData,
    ) -> io::Result<()> {
        let writer = &mut self.writer;
        match value_data.kind() {
            ValueKind::Integer(_) => {}

            ValueKind::Call(call) => {
                let args = call.args();

                // Load arguments into regs or stack
                for (i, &arg) in args.iter().enumerate() {
                    if i < 8 {
                        self.load_value_to_reg(arg, &format!("a{}", i), stack_frame)?;
                    } else {
                        self.load_value_to_reg(arg, "t0", stack_frame)?;
                        let offset = (i as i32 - 8) * WORD_SIZE;
                        self.prepare_addr(offset, "t1")?;
                        let addr = self.get_addr_str(offset, "t1");
                        writer.write_inst("sw", &["t0", &addr])?;
                    }
                }

                // Call the function
                let callee = call.callee();
                let callee_name = self.program.func(callee).name().replace("@", "");
                writer.write_inst("call", &[&callee_name])?;

                // Save return value if there is one
                if !value_data.ty().is_unit() {
                    self.save_value_from_reg(value, "a0", stack_frame)?;
                }
            }

            ValueKind::Return(ret) => {
                // Load return value into a0 if exists
                if let Some(ret_value) = ret.value() {
                    self.load_value_to_reg(ret_value, "a0", stack_frame)?; 
                }
                self.restore_caller_saved_regs(stack_frame)?;
                self.generate_epilogue(stack_frame)?;
                writer.write_inst("ret", &[])?;
            }

            ValueKind::Binary(bin) => {
                self.load_value_to_reg(bin.lhs(), "t0", stack_frame)?;
                self.load_value_to_reg(bin.rhs(), "t1", stack_frame)?;

                let op_str = map_binary_op(bin.op());
                match bin.op() {
                    KoopaBinaryOp::Le => {
                        writer.write_inst("sgt", &["t0", "t0", "t1"])?; // t0 = (lhs > rhs)
                        writer.write_inst("seqz", &["t0", "t0"])?; // t0 = (t0 == 0) => !(lhs > rhs) => lhs <= rhs
                    }
                    KoopaBinaryOp::Ge => {
                        writer.write_inst("slt", &["t0", "t0", "t1"])?;
                        writer.write_inst("seqz", &["t0", "t0"])?;
                    }
                    KoopaBinaryOp::Eq => {
                        writer.write_inst("xor", &["t0", "t0", "t1"])?;
                        writer.write_inst("seqz", &["t0", "t0"])?;
                    }
                    KoopaBinaryOp::NotEq => {
                        writer.write_inst("xor", &["t0", "t0", "t1"])?;
                        writer.write_inst("snez", &["t0", "t0"])?;
                    }
                    _ => {
                        // Regular binary operations
                        if let Some(op) = op_str {
                            writer.write_inst(op, &["t0", "t0", "t1"])?;
                        } else {
                            unreachable!("Unknown binary op");
                        }
                    }
                }
                self.save_value_from_reg(value, "t0", stack_frame)?;
            }

            ValueKind::Alloc(_) => {
                // Allocation handled in stack frame setup.
                // Does nothing here
            }

            ValueKind::Store(store) => {
                let store_value = store.value();
                let dest = store.dest();
                if dest.is_global() {
                    let global_name = self
                        .program
                        .borrow_value(dest)
                        .name()
                        .as_ref()
                        .unwrap()
                        .replace("@", "");
                    self.load_value_to_reg(store_value, "t0", stack_frame)?;
                    writer.write_inst("la", &["t1", &global_name])?;
                    writer.write_inst("sw", &["t0", "0(t1)"])?;
                    return Ok(());
                }
                let offset = stack_frame.get_stack_offset(dest);
                self.load_value_to_reg(store_value, "t0", stack_frame)?;
                self.prepare_addr(offset, "t1")?;
                let addr: String = self.get_addr_str(offset, "t1");
                writer.write_inst("sw", &["t0", &addr])?;
            }

            ValueKind::Load(load) => {
                let src = load.src();
                if src.is_global() {
                    let global_name = self
                        .program
                        .borrow_value(src)
                        .name()
                        .as_ref()
                        .unwrap()
                        .replace("@", "");
                    writer.write_inst("la", &["t0", &global_name])?;
                    writer.write_inst("lw", &["t0", "0(t0)"])?;
                    self.save_value_from_reg(value, "t0", stack_frame)?;
                } else {
                    let offset = stack_frame.get_stack_offset(src);

                    self.prepare_addr(offset, "t0")?;
                    let addr: String = self.get_addr_str(offset, "t0");
                    writer.write_inst("lw", &["t0", &addr])?;

                    self.save_value_from_reg(value, "t0", stack_frame)?;
                }
            }

            ValueKind::Branch(branch) => {
                let cond = branch.cond();
                let true_bb = branch.true_bb();
                let false_bb = branch.false_bb();

                self.load_value_to_reg(cond, "t0", stack_frame)?;
                let true_bb_name = Self::get_bb_name(func, true_bb);
                let false_bb_name = Self::get_bb_name(func, false_bb);
                writer.write_inst("bnez", &["t0", &true_bb_name])?;
                writer.write_inst("j", &[&false_bb_name])?;
            }

            ValueKind::Jump(jump) => {
                let target_bb = jump.target();
                let target_bb_name = Self::get_bb_name(func, target_bb);
                writer.write_inst("j", &[&target_bb_name])?;
            }

            _ => {
                panic!("Unsupported instruction in RISC-V generation");
            }
        }
        Ok(())
    }
}

fn map_binary_op(op: KoopaBinaryOp) -> Option<&'static str> {
    match op {
        // All instructions are in the format `op rd, rs1, rs2`
        KoopaBinaryOp::Add => Some("add"),
        KoopaBinaryOp::Sub => Some("sub"),
        KoopaBinaryOp::Mul => Some("mul"),
        KoopaBinaryOp::Div => Some("div"),
        KoopaBinaryOp::Mod => Some("rem"),
        KoopaBinaryOp::And => Some("and"),
        KoopaBinaryOp::Or => Some("or"),
        KoopaBinaryOp::Lt => Some("slt"),
        KoopaBinaryOp::Gt => Some("sgt"),
        KoopaBinaryOp::Sar => Some("sra"),
        KoopaBinaryOp::Shl => Some("sll"),
        KoopaBinaryOp::Shr => Some("srl"),
        KoopaBinaryOp::Xor => Some("xor"),
        KoopaBinaryOp::Eq | KoopaBinaryOp::NotEq | KoopaBinaryOp::Ge | KoopaBinaryOp::Le => None,
    }
}
