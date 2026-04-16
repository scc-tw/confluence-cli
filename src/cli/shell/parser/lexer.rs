use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Word(String),
    Pipe,
}

pub fn lex(line: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some(active) => match ch {
                '\\' if active == '"' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                q if q == active => quote = None,
                _ => current.push(ch),
            },
            None => match ch {
                '"' | '\'' => quote = Some(ch),
                '|' => {
                    flush_word(&mut tokens, &mut current);
                    tokens.push(Token::Pipe);
                }
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                c if c.is_whitespace() => flush_word(&mut tokens, &mut current),
                _ => current.push(ch),
            },
        }
    }

    if quote.is_some() {
        return Err(ConfluenceCliError::Config(
            "shell input has unmatched quotes".to_owned(),
        ));
    }

    flush_word(&mut tokens, &mut current);
    Ok(tokens)
}

fn flush_word(tokens: &mut Vec<Token>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(Token::Word(std::mem::take(current)));
    }
}
