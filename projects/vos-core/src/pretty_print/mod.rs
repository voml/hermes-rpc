use std::fmt::{Arguments, Display, Formatter, Write};

use indenter::CodeFormatter;

use crate::{Codegen, Environment, Project, Validation};

pub struct PrettyPrinter {}

impl Codegen for PrettyPrinter {
    type Output = String;

    fn generate(&self, project: &Project) -> Validation<Self::Output> {
        Validation::ok(project.to_string())
    }
}

struct Context<'s, T>
where
    T: Write,
{
    buffer: CodeFormatter<'s, T>,
}

impl<'s, T: Write> Write for Context<'s, T> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buffer.write_str(s)
    }
    fn write_char(&mut self, c: char) -> std::fmt::Result {
        self.buffer.write_char(c)
    }
    fn write_fmt(self: &mut Self, args: Arguments<'_>) -> std::fmt::Result {
        self.buffer.write_fmt(args)
    }
}

impl<'s, T> Context<'s, T>
where
    T: Write,
{
    pub fn new(s: &'s mut T) -> Context<'s, T> {
        Self { buffer: CodeFormatter::new(s, "    ") }
    }
    pub fn indent(&mut self) {
        self.buffer.indent(1)
    }
    pub fn dedent(&mut self) {
        self.buffer.dedent(1)
    }

    pub fn visit_root(&mut self, root: &Project) -> std::fmt::Result {
        write!(self, "\n{}", root.description)?;
        writeln!(self, "\nservice {{")?;
        self.indent();
        writeln!(self, "\nedition: \"{}\",", root.edition)?;
        writeln!(self, "\nversion: \"{}\",", root.version)?;
        self.dedent();
        writeln!(self, "\n}}")?;
        writeln!(self, "\n")?;
        for env in &root.environments {
            self.visit_env(env)?
        }
        Ok(())
    }
    pub fn visit_env(&mut self, env: &Environment) -> std::fmt::Result {
        write!(self, "\n{}", env.document)?;
        writeln!(self, "\nenvironment {{")?;
        self.indent();
        writeln!(self, "\nhost: \"{}\"", env.host)?;

        self.dedent();
        writeln!(self, "}}")?;
        Ok(())
    }
}

impl Display for Project {
    /// ```vos
    /// /// dies
    /// service {
    ///     a:x
    /// }
    /// ```
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut ctx = Context::new(f);
        ctx.visit_root(self)
    }
}
