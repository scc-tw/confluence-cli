#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellLine {
    pub pipeline: Pipeline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub commands: Vec<SimpleCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleCommand {
    pub argv: Vec<String>,
}
