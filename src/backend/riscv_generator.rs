use crate::backend::asm_writer::AsmWriter;
use crate::backend::stack_frame::StackFrame;
use koopa::ir::entities::*;
use koopa::ir::{values::BinaryOp as KoopaBinaryOp, *};
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
        let program = self.program;

        self.writer.write_directive("data", &[], false)?;
        for &global in program.inst_layout() {
            let global_data = program.borrow_value(global);
            let name = global_data.name().as_ref().unwrap().replace("@", "");

            self.writer.write_directive("globl", &[&name], true)?;
            self.writer.write_label(&name)?;

            match global_data.kind() {
                ValueKind::GlobalAlloc(alloc) => {
                    let init = alloc.init();
                    let init_data = program.borrow_value(init);
                    match init_data.kind() {
                        ValueKind::Integer(int) => {
                            self.writer.write_directive(
                                "word",
                                &[&int.value().to_string()],
                                true,
                            )?;
                        }
                        ValueKind::ZeroInit(_) => {
                            self.writer
                                .write_directive("zero", &[&WORD_SIZE.to_string()], true)?;
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

        self.writer.write_directive("text", &[], false)?;
        for &func in program.func_layout() {
            let func_data = program.func(func);
            // Skip function declarations (none entry basic block)
            if func_data.layout().entry_bb().is_none() {
                continue;
            }
            let mut func_gen = FunctionGenerator::new(self, func_data);
            func_gen.generate_function()?;
        }
        Ok(())
    }
}

struct FunctionGenerator<'a, 'b, W: Write> {
    gen: &'a mut RiscvGenerator<'b, W>,
    func: &'b FunctionData,
    stack_frame: StackFrame,
}

impl<'a, 'b, W: Write> FunctionGenerator<'a, 'b, W> {
    pub fn new(riscv_gen: &'a mut RiscvGenerator<'b, W>, func: &'b FunctionData) -> Self {
        let mut stack_frame = StackFrame::new();
        stack_frame.initialize(func);
        Self {
            gen: riscv_gen,
            func,
            stack_frame,
        }
    }

    fn generate_function(&mut self) -> io::Result<()> {
        // Function name starts with an '@'
        let name = self.func.name().replace("@", "");
        self.gen.writer.write_directive("globl", &[&name], true)?;
        self.gen.writer.write_label(&name)?;

        // Stack frame setup
        self.generate_prologue()?;
        self.save_caller_saved_regs()?;

        // Generate code for each basic block
        for (&bb, node) in self.func.layout().bbs() {
            // node is a &BasicBlockNode
            let bb_name = self.get_bb_name(bb);
            self.gen.writer.write_label(&bb_name)?;

            // Generate code for each instruction in the basic block
            for &inst in node.insts().keys() {
                self.generate_instruction(inst)?;
            }
        }
        Ok(())
    }

    fn get_bb_name(&self, bb: BasicBlock) -> String {
        self.func
            .dfg()
            .bb(bb)
            .name()
            .as_ref()
            .unwrap()
            .replace("%", "")
    }

    fn generate_prologue(&mut self) -> io::Result<()> {
        let stack_size = self.stack_frame.get_stack_size();
        if stack_size == 0 {
            return Ok(());
        }
        let offset = (-stack_size).to_string();
        if stack_size > MAX_IMM_12 {
            self.gen.writer.write_inst("li", &["t0", &offset])?;
            self.gen.writer.write_inst("add", &["sp", "sp", "t0"])?;
        } else {
            self.gen.writer.write_inst("addi", &["sp", "sp", &offset])?;
        }
        Ok(())
    }

    fn generate_epilogue(&mut self) -> io::Result<()> {
        let stack_size = self.stack_frame.get_stack_size();
        if stack_size == 0 {
            return Ok(());
        }
        let offset = stack_size.to_string();
        if stack_size > MAX_IMM_12 {
            self.gen.writer.write_inst("li", &["t0", &offset])?;
            self.gen.writer.write_inst("add", &["sp", "sp", "t0"])?;
        } else {
            self.gen.writer.write_inst("addi", &["sp", "sp", &offset])?;
        }
        Ok(())
    }

    fn prepare_addr(&mut self, offset: i32, tmp_reg: &str) -> io::Result<()> {
        if offset > MAX_IMM_12 || offset < -MAX_IMM_12 - 1 {
            self.gen
                .writer
                .write_inst("li", &["t0", &offset.to_string()])?;
            self.gen.writer.write_inst("add", &[tmp_reg, "sp", "t0"])?;
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

    fn save_caller_saved_regs(&mut self) -> io::Result<()> {
        let Some(ra_offset) = self.stack_frame.get_ra_offset() else {
            return Ok(());
        };
        self.prepare_addr(ra_offset, "t0")?;
        let addr = self.get_addr_str(ra_offset, "t0");
        self.gen.writer.write_inst("sw", &["ra", &addr])
    }

    fn restore_caller_saved_regs(&mut self) -> io::Result<()> {
        let Some(ra_offset) = self.stack_frame.get_ra_offset() else {
            return Ok(());
        };
        self.prepare_addr(ra_offset, "t0")?;
        let addr = self.get_addr_str(ra_offset, "t0");
        self.gen.writer.write_inst("lw", &["ra", &addr])
    }

    fn load_global_value_to_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        let global_name = self
            .gen
            .program
            .borrow_value(value)
            .name()
            .as_ref()
            .unwrap()
            .replace("@", "");
        self.gen.writer.write_inst("la", &[reg, &global_name])?;
        self.gen
            .writer
            .write_inst("lw", &[reg, &("0(".to_string() + reg + ")")])
    }

    fn load_local_value_to_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        let value_data = self.func.dfg().value(value);
        match value_data.kind() {
            ValueKind::Integer(int) => {
                if int.value() == 0 {
                    self.gen.writer.write_inst("mv", &[reg, "x0"])
                } else {
                    self.gen
                        .writer
                        .write_inst("li", &[reg, &int.value().to_string()])
                }
            }
            ValueKind::FuncArgRef(arg) => {
                let arg_index = arg.index() as i32;
                if arg_index < 8 {
                    self.gen
                        .writer
                        .write_inst("mv", &[reg, &format!("a{}", arg_index)])
                } else {
                    let offset = (arg_index - 8) * WORD_SIZE + self.stack_frame.get_stack_size();
                    self.prepare_addr(offset, reg)?;
                    let addr: String = self.get_addr_str(offset, reg);
                    self.gen.writer.write_inst("lw", &[reg, &addr])
                }
            }
            // Result of other instructions
            // They should have been already stored on the stack
            _ => {
                let offset = self.stack_frame.get_stack_offset(value);
                self.prepare_addr(offset, "t0")?;
                let addr: String = self.get_addr_str(offset, "t0");
                self.gen.writer.write_inst("lw", &[reg, &addr])
            }
        }
    }

    fn load_value_to_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        if value.is_global() {
            self.load_global_value_to_reg(value, reg)
        } else {
            self.load_local_value_to_reg(value, reg)
        }
    }

    fn save_global_value_from_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        let global_name = self
            .gen
            .program
            .borrow_value(value)
            .name()
            .as_ref()
            .unwrap()
            .replace("@", "");
        self.gen.writer.write_inst("la", &["t0", &global_name])?;
        self.gen.writer.write_inst("sw", &[reg, "0(t0)"])
    }

    fn save_local_value_from_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        let offset = self.stack_frame.get_stack_offset(value);
        self.prepare_addr(offset, "t0")?;
        let addr: String = self.get_addr_str(offset, "t0");
        self.gen.writer.write_inst("sw", &[reg, &addr])
    }

    fn save_value_from_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        if value.is_global() {
            self.save_global_value_from_reg(value, reg)
        } else {
            self.save_local_value_from_reg(value, reg)
        }
    }

    fn generate_instruction(&mut self, value: Value) -> io::Result<()> {
        let value_kind = self.get_value_kind(value);
        match value_kind {
            ValueKind::Integer(_) => {}

            ValueKind::Call(call) => {
                let args = call.args();

                // Load arguments into regs or stack
                for (i, &arg) in args.iter().enumerate() {
                    if i < 8 {
                        self.load_value_to_reg(arg, &format!("a{}", i))?;
                    } else {
                        self.load_value_to_reg(arg, "t0")?;
                        let offset = (i as i32 - 8) * WORD_SIZE;
                        self.prepare_addr(offset, "t1")?;
                        let addr = self.get_addr_str(offset, "t1");
                        self.gen.writer.write_inst("sw", &["t0", &addr])?;
                    }
                }

                // Call the function
                let callee = call.callee();
                let callee_name = self.gen.program.func(callee).name().replace("@", "");
                self.gen.writer.write_inst("call", &[&callee_name])?;

                // Save return value if there is one
                let value_type = self.get_value_type(value);
                if !value_type.is_unit() {
                    self.save_value_from_reg(value, "a0")?;
                }
            }

            ValueKind::Return(ret) => {
                // Load return value into a0 if exists
                if let Some(ret_value) = ret.value() {
                    self.load_value_to_reg(ret_value, "a0")?;
                }
                self.restore_caller_saved_regs()?;
                self.generate_epilogue()?;
                self.gen.writer.write_inst("ret", &[])?;
            }

            ValueKind::Binary(bin) => {
                self.load_value_to_reg(bin.lhs(), "t0")?;
                self.load_value_to_reg(bin.rhs(), "t1")?;

                let op_str = map_binary_op(bin.op());
                match bin.op() {
                    KoopaBinaryOp::Le => {
                        self.gen.writer.write_inst("sgt", &["t0", "t0", "t1"])?; // t0 = (lhs > rhs)
                        self.gen.writer.write_inst("seqz", &["t0", "t0"])?; // t0 = (t0 == 0) => !(lhs > rhs) => lhs <= rhs
                    }
                    KoopaBinaryOp::Ge => {
                        self.gen.writer.write_inst("slt", &["t0", "t0", "t1"])?;
                        self.gen.writer.write_inst("seqz", &["t0", "t0"])?;
                    }
                    KoopaBinaryOp::Eq => {
                        self.gen.writer.write_inst("xor", &["t0", "t0", "t1"])?;
                        self.gen.writer.write_inst("seqz", &["t0", "t0"])?;
                    }
                    KoopaBinaryOp::NotEq => {
                        self.gen.writer.write_inst("xor", &["t0", "t0", "t1"])?;
                        self.gen.writer.write_inst("snez", &["t0", "t0"])?;
                    }
                    _ => {
                        // Regular binary operations
                        if let Some(op) = op_str {
                            self.gen.writer.write_inst(op, &["t0", "t0", "t1"])?;
                        } else {
                            unreachable!("Unknown binary op");
                        }
                    }
                }
                self.save_value_from_reg(value, "t0")?;
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
                        .gen
                        .program
                        .borrow_value(dest)
                        .name()
                        .as_ref()
                        .unwrap()
                        .replace("@", "");
                    self.load_value_to_reg(store_value, "t0")?;
                    self.gen.writer.write_inst("la", &["t1", &global_name])?;
                    self.gen.writer.write_inst("sw", &["t0", "0(t1)"])?;
                    return Ok(());
                }
                let offset = self.stack_frame.get_stack_offset(dest);
                self.load_value_to_reg(store_value, "t0")?;
                self.prepare_addr(offset, "t1")?;
                let addr: String = self.get_addr_str(offset, "t1");
                self.gen.writer.write_inst("sw", &["t0", &addr])?;
            }

            ValueKind::Load(load) => {
                let src = load.src();
                if src.is_global() {
                    let global_name = self
                        .gen
                        .program
                        .borrow_value(src)
                        .name()
                        .as_ref()
                        .unwrap()
                        .replace("@", "");
                    self.gen.writer.write_inst("la", &["t0", &global_name])?;
                    self.gen.writer.write_inst("lw", &["t0", "0(t0)"])?;
                    self.save_value_from_reg(value, "t0")?;
                } else {
                    let offset = self.stack_frame.get_stack_offset(src);

                    self.prepare_addr(offset, "t0")?;
                    let addr: String = self.get_addr_str(offset, "t0");
                    self.gen.writer.write_inst("lw", &["t0", &addr])?;

                    self.save_value_from_reg(value, "t0")?;
                }
            }

            ValueKind::Branch(branch) => {
                let cond = branch.cond();
                let true_bb = branch.true_bb();
                let false_bb = branch.false_bb();

                self.load_value_to_reg(cond, "t0")?;
                let true_bb_name = self.get_bb_name(true_bb);
                let false_bb_name = self.get_bb_name(false_bb);
                self.gen.writer.write_inst("bnez", &["t0", &true_bb_name])?;
                self.gen.writer.write_inst("j", &[&false_bb_name])?;
            }

            ValueKind::Jump(jump) => {
                let target_bb = jump.target();
                let target_bb_name = self.get_bb_name(target_bb);
                self.gen.writer.write_inst("j", &[&target_bb_name])?;
            }

            _ => {
                panic!("Unsupported instruction in RISC-V generation");
            }
        }
        Ok(())
    }

    fn get_value_kind(&self, value: Value) -> ValueKind {
        self.func.dfg().value(value).kind().clone()
    }

    fn get_value_type(&self, value: Value) -> Type {
        self.func.dfg().value(value).ty().clone()
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
