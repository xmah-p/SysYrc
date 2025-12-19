use std::io::{self, Write};

pub struct AsmWriter<W: Write> {
    writer: W,
}

impl<W: Write> AsmWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn write_inst(&mut self, inst: &str, args: &[&str]) -> io::Result<()> {
        write!(self.writer, "    {}", inst)?;
        if !args.is_empty() {
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    write!(self.writer, ",")?;
                }
                write!(self.writer, " {}", arg)?;
            }
        }
        writeln!(self.writer)
    }

    pub fn write_label(&mut self, label: &str) -> io::Result<()> {
        writeln!(self.writer, "{}:", label)
    }

    pub fn write_directive(&mut self, directive: &str, args: &[&str], indent: bool) -> io::Result<()> {
        if indent {
            write!(self.writer, "    .{}", directive)?;
        } else {
            write!(self.writer, ".{}", directive)?;
        }
        if !args.is_empty() {
            write!(self.writer, " ")?;
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    write!(self.writer, ", ")?;
                }
                write!(self.writer, "{}", arg)?;
            }
        }
        writeln!(self.writer)
    }
}