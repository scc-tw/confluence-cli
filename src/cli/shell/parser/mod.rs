mod lexer;
mod syntax;

pub use syntax::{Pipeline, ShellLine, SimpleCommand};

use crate::support::{ConfluenceCliError, Result};
use lexer::{lex, Token};

pub struct ShellParser;

impl ShellParser {
    pub fn parse(line: &str) -> Result<ShellLine> {
        let tokens = lex(line)?;
        let mut commands = Vec::new();
        let mut current = Vec::new();

        for token in tokens {
            match token {
                Token::Word(word) => current.push(word),
                Token::Pipe => {
                    if current.is_empty() {
                        return Err(ConfluenceCliError::Config(
                            "pipeline contains an empty command".to_owned(),
                        ));
                    }
                    commands.push(SimpleCommand { argv: current });
                    current = Vec::new();
                }
            }
        }

        if current.is_empty() {
            return Err(ConfluenceCliError::Config(
                "pipeline contains an empty command".to_owned(),
            ));
        }
        commands.push(SimpleCommand { argv: current });

        Ok(ShellLine {
            pipeline: Pipeline { commands },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ShellParser;

    #[test]
    fn parses_quoted_argument() {
        let parsed = ShellParser::parse("ls \"People Docs\"").expect("parser should succeed");
        assert_eq!(parsed.pipeline.commands.len(), 1);
        assert_eq!(parsed.pipeline.commands[0].argv, vec!["ls", "People Docs"]);
    }

    #[test]
    fn parses_pipeline() {
        let parsed = ShellParser::parse("ls ALPHA | grep Project").expect("parser should succeed");
        assert_eq!(parsed.pipeline.commands.len(), 2);
        assert_eq!(parsed.pipeline.commands[0].argv, vec!["ls", "ALPHA"]);
        assert_eq!(parsed.pipeline.commands[1].argv, vec!["grep", "Project"]);
    }

    #[test]
    fn rejects_empty_pipeline_stage() {
        let error =
            ShellParser::parse("ls || grep x").expect_err("parser should reject empty stage");
        assert!(error.to_string().contains("empty command"));
    }
}
