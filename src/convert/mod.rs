use diffy::{Patch, apply};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use crate::domain::BodyFormat;
use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    BulletList(Vec<Vec<Inline>>),
    RawStorage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Emphasis(String),
    Strong(String),
    Code(String),
    Link { text: String, url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedBundleMetadata {
    pub page_id: Option<u64>,
    pub title: Option<String>,
    pub version: Option<u32>,
    pub body_hash: String,
}

pub fn convert_text(input: &str, from: BodyFormat, to: BodyFormat) -> Result<String> {
    if from == to {
        return Ok(input.to_owned());
    }

    let document = match from {
        BodyFormat::Markdown => markdown_to_document(input),
        BodyFormat::Storage | BodyFormat::Html => storage_to_document(input)?,
        BodyFormat::Text => Document {
            blocks: vec![Block::Paragraph(vec![Inline::Text(
                input.trim().to_owned(),
            )])],
        },
    };

    match to {
        BodyFormat::Markdown => Ok(document_to_markdown(&document)),
        BodyFormat::Storage | BodyFormat::Html => Ok(document_to_storage(&document)),
        BodyFormat::Text => Ok(document_to_text(&document)),
    }
}

pub fn apply_unified_patch(base: &str, patch_text: &str) -> Result<String> {
    let patch: Patch<'_, str> = Patch::from_str(patch_text)
        .map_err(|error| ConfluenceCliError::Config(format!("invalid patch: {error}")))?;
    apply(base, &patch)
        .map_err(|error| ConfluenceCliError::Config(format!("patch failed: {error}")))
}

pub fn export_managed_bundle(
    directory: &Path,
    metadata: &ManagedBundleMetadata,
    markdown: &str,
) -> Result<()> {
    export_bundle_file(directory, metadata, "page.md", markdown)
}

pub fn export_bundle_file(
    directory: &Path,
    metadata: &ManagedBundleMetadata,
    file_name: &str,
    content: &str,
) -> Result<()> {
    fs::create_dir_all(directory.join(".confluence"))?;
    fs::write(directory.join(file_name), content)?;
    fs::write(
        directory.join(".confluence").join("page.json"),
        serde_json::to_string_pretty(metadata)?,
    )?;
    Ok(())
}

pub fn build_bundle_metadata(
    page_id: Option<u64>,
    title: Option<String>,
    version: Option<u32>,
    body: &str,
) -> ManagedBundleMetadata {
    ManagedBundleMetadata {
        page_id,
        title,
        version,
        body_hash: stable_hash(body),
    }
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn markdown_to_document(input: &str) -> Document {
    let mut blocks = Vec::new();
    let mut lines = input.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(level) = heading_level(trimmed) {
            let content = trimmed[level as usize + 1..].trim();
            blocks.push(Block::Heading {
                level,
                content: parse_inline(content),
            });
            continue;
        }

        if trimmed.starts_with("```") {
            let language = trimmed.trim_start_matches("```").trim();
            let language = if language.is_empty() {
                None
            } else {
                Some(language.to_owned())
            };

            let mut code = String::new();
            for code_line in lines.by_ref() {
                if code_line.trim() == "```" {
                    break;
                }

                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(code_line);
            }

            blocks.push(Block::CodeBlock { language, code });
            continue;
        }

        if trimmed.starts_with("- ") {
            let mut items = vec![parse_inline(trimmed.trim_start_matches("- ").trim())];
            while let Some(next) = lines.peek() {
                if next.trim().starts_with("- ") {
                    let item = lines.next().expect("peeked line should exist");
                    items.push(parse_inline(item.trim().trim_start_matches("- ").trim()));
                } else if next.trim().is_empty() {
                    lines.next();
                    break;
                } else {
                    break;
                }
            }
            blocks.push(Block::BulletList(items));
            continue;
        }

        let mut paragraph = String::from(trimmed);
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() {
                lines.next();
                break;
            }

            if heading_level(next.trim()).is_some()
                || next.trim().starts_with("- ")
                || next.trim().starts_with("```")
            {
                break;
            }

            paragraph.push(' ');
            paragraph.push_str(lines.next().expect("peeked line should exist").trim());
        }

        blocks.push(Block::Paragraph(parse_inline(&paragraph)));
    }

    Document { blocks }
}

fn heading_level(line: &str) -> Option<u8> {
    for level in 1..=6 {
        let marker = "#".repeat(level as usize);
        if line.starts_with(&format!("{marker} ")) {
            return Some(level);
        }
    }
    None
}

fn parse_inline(input: &str) -> Vec<Inline> {
    let mut remaining = input;
    let mut inlines = Vec::new();

    while !remaining.is_empty() {
        if let Some((before, text, url, after)) = take_link(remaining) {
            push_text(&mut inlines, before);
            inlines.push(Inline::Link {
                text: text.to_owned(),
                url: url.to_owned(),
            });
            remaining = after;
            continue;
        }

        if let Some((before, inner, after)) = take_delimited(remaining, "**") {
            push_text(&mut inlines, before);
            inlines.push(Inline::Strong(inner.to_owned()));
            remaining = after;
            continue;
        }

        if let Some((before, inner, after)) = take_delimited(remaining, "`") {
            push_text(&mut inlines, before);
            inlines.push(Inline::Code(inner.to_owned()));
            remaining = after;
            continue;
        }

        if let Some((before, inner, after)) = take_delimited(remaining, "*") {
            push_text(&mut inlines, before);
            inlines.push(Inline::Emphasis(inner.to_owned()));
            remaining = after;
            continue;
        }

        push_text(&mut inlines, remaining);
        break;
    }

    inlines
}

fn take_delimited<'a>(input: &'a str, marker: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let start = input.find(marker)?;
    let rest = &input[start + marker.len()..];
    let end = rest.find(marker)?;
    Some((&input[..start], &rest[..end], &rest[end + marker.len()..]))
}

fn take_link(input: &str) -> Option<(&str, &str, &str, &str)> {
    let open = input.find('[')?;
    let close = input[open + 1..].find(']')? + open + 1;
    let paren_open = input[close + 1..].find('(')? + close + 1;
    if paren_open != close + 1 {
        return None;
    }
    let paren_close = input[paren_open + 1..].find(')')? + paren_open + 1;
    Some((
        &input[..open],
        &input[open + 1..close],
        &input[paren_open + 1..paren_close],
        &input[paren_close + 1..],
    ))
}

fn push_text(inlines: &mut Vec<Inline>, text: &str) {
    if !text.is_empty() {
        inlines.push(Inline::Text(text.to_owned()));
    }
}

pub fn document_to_markdown(document: &Document) -> String {
    let mut output = Vec::new();
    for block in &document.blocks {
        match block {
            Block::Heading { level, content } => {
                output.push(format!(
                    "{} {}",
                    "#".repeat(*level as usize),
                    inline_to_markdown(content)
                ));
            }
            Block::Paragraph(content) => output.push(inline_to_markdown(content)),
            Block::CodeBlock { language, code } => {
                let mut block_output = String::from("```");
                if let Some(language) = language {
                    block_output.push_str(language);
                }
                block_output.push('\n');
                block_output.push_str(code);
                block_output.push_str("\n```");
                output.push(block_output);
            }
            Block::BulletList(items) => {
                for item in items {
                    output.push(format!("- {}", inline_to_markdown(item)));
                }
            }
            Block::RawStorage(raw) => output.push(format!("```storage\n{}\n```", raw)),
        }
    }

    output.join("\n\n")
}

fn inline_to_markdown(inlines: &[Inline]) -> String {
    let mut output = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) => output.push_str(text),
            Inline::Emphasis(text) => output.push_str(&format!("*{text}*")),
            Inline::Strong(text) => output.push_str(&format!("**{text}**")),
            Inline::Code(text) => output.push_str(&format!("`{text}`")),
            Inline::Link { text, url } => output.push_str(&format!("[{text}]({url})")),
        }
    }
    output
}

pub fn storage_to_document(input: &str) -> Result<Document> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut blocks = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.local_name().as_ref()).to_string();
                match name.as_str() {
                    "p" => {
                        blocks.push(Block::Paragraph(read_inline_storage(
                            &mut reader,
                            event.name().as_ref().to_vec(),
                        )?));
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = name[1..].parse::<u8>().unwrap_or(1);
                        blocks.push(Block::Heading {
                            level,
                            content: read_inline_storage(
                                &mut reader,
                                event.name().as_ref().to_vec(),
                            )?,
                        });
                    }
                    "pre" => {
                        blocks.push(Block::CodeBlock {
                            language: None,
                            code: inline_to_text(&read_inline_storage(
                                &mut reader,
                                event.name().as_ref().to_vec(),
                            )?),
                        });
                    }
                    "ul" => {
                        let items = read_list_items(&mut reader, event.name().as_ref().to_vec())?;
                        blocks.push(Block::BulletList(items));
                    }
                    other => {
                        let raw = inline_to_text(&read_inline_storage(
                            &mut reader,
                            event.name().as_ref().to_vec(),
                        )?);
                        blocks.push(Block::RawStorage(format!("<{other}>{raw}</{other}>")));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(ConfluenceCliError::Config(format!(
                    "invalid storage body: {error}"
                )));
            }
        }
    }

    Ok(Document { blocks })
}

fn read_list_items(reader: &mut Reader<&[u8]>, end_tag: Vec<u8>) -> Result<Vec<Vec<Inline>>> {
    let mut items = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if event.name().as_ref() == b"li" => {
                items.push(read_inline_storage(reader, event.name().as_ref().to_vec())?);
            }
            Ok(Event::End(event)) if event.name().as_ref() == end_tag.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(ConfluenceCliError::Config(
                    "invalid storage body: unexpected EOF while reading list".to_owned(),
                ));
            }
            Ok(_) => {}
            Err(error) => {
                return Err(ConfluenceCliError::Config(format!(
                    "invalid storage body: {error}"
                )));
            }
        }
    }

    Ok(items)
}

fn read_inline_storage(reader: &mut Reader<&[u8]>, end_tag: Vec<u8>) -> Result<Vec<Inline>> {
    let mut inlines = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = String::from_utf8_lossy(event.local_name().as_ref()).to_string();
                let nested = read_inline_storage(reader, event.name().as_ref().to_vec())?;
                let text = inline_to_text(&nested);
                match tag.as_str() {
                    "strong" | "b" => inlines.push(Inline::Strong(text)),
                    "em" | "i" => inlines.push(Inline::Emphasis(text)),
                    "code" => inlines.push(Inline::Code(text)),
                    "a" => {
                        let url = event
                            .attributes()
                            .flatten()
                            .find(|attribute| attribute.key.as_ref() == b"href")
                            .map(|attribute| {
                                String::from_utf8_lossy(attribute.value.as_ref()).to_string()
                            })
                            .unwrap_or_default();
                        if url.is_empty() {
                            push_text(&mut inlines, &text);
                        } else {
                            inlines.push(Inline::Link { text, url });
                        }
                    }
                    _ => push_text(&mut inlines, &text),
                }
            }
            Ok(Event::Empty(event)) => {
                if event.name().as_ref() == b"br" {
                    push_text(&mut inlines, "\n");
                }
            }
            Ok(Event::Text(event)) => {
                let text = event
                    .decode()
                    .map(|value| value.into_owned())
                    .unwrap_or_default();
                push_text(&mut inlines, &text);
            }
            Ok(Event::CData(event)) => {
                let text = String::from_utf8_lossy(event.as_ref()).to_string();
                push_text(&mut inlines, &text);
            }
            Ok(Event::End(event)) if event.name().as_ref() == end_tag.as_slice() => break,
            Ok(Event::Eof) => {
                return Err(ConfluenceCliError::Config(
                    "invalid storage body: unexpected EOF while reading inline content".to_owned(),
                ));
            }
            Ok(_) => {}
            Err(error) => {
                return Err(ConfluenceCliError::Config(format!(
                    "invalid storage body: {error}"
                )));
            }
        }
    }

    Ok(inlines)
}

pub fn document_to_storage(document: &Document) -> String {
    let mut output = String::new();
    for block in &document.blocks {
        match block {
            Block::Heading { level, content } => output.push_str(&format!(
                "<h{level}>{}</h{level}>",
                inline_to_storage(content)
            )),
            Block::Paragraph(content) => {
                output.push_str(&format!("<p>{}</p>", inline_to_storage(content)));
            }
            Block::CodeBlock { code, .. } => {
                output.push_str(&format!("<pre><code>{}</code></pre>", escape_html(code)));
            }
            Block::BulletList(items) => {
                output.push_str("<ul>");
                for item in items {
                    output.push_str(&format!("<li>{}</li>", inline_to_storage(item)));
                }
                output.push_str("</ul>");
            }
            Block::RawStorage(raw) => output.push_str(raw),
        }
    }
    output
}

fn inline_to_storage(inlines: &[Inline]) -> String {
    let mut output = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) => output.push_str(&escape_html(text)),
            Inline::Emphasis(text) => output.push_str(&format!("<em>{}</em>", escape_html(text))),
            Inline::Strong(text) => {
                output.push_str(&format!("<strong>{}</strong>", escape_html(text)))
            }
            Inline::Code(text) => output.push_str(&format!("<code>{}</code>", escape_html(text))),
            Inline::Link { text, url } => output.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape_html(url),
                escape_html(text)
            )),
        }
    }
    output
}

pub fn document_to_text(document: &Document) -> String {
    let mut output = Vec::new();
    for block in &document.blocks {
        match block {
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                output.push(inline_to_text(content));
            }
            Block::CodeBlock { code, .. } => output.push(code.clone()),
            Block::BulletList(items) => {
                output.extend(
                    items
                        .iter()
                        .map(|item| format!("- {}", inline_to_text(item))),
                );
            }
            Block::RawStorage(raw) => output.push(raw.clone()),
        }
    }

    output.join("\n")
}

fn inline_to_text(inlines: &[Inline]) -> String {
    let mut output = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text)
            | Inline::Emphasis(text)
            | Inline::Strong(text)
            | Inline::Code(text) => output.push_str(text),
            Inline::Link { text, url } => output.push_str(&format!("{text} ({url})")),
        }
    }
    output
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn markdown_round_trip_preserves_supported_blocks() {
        let source = "# Title\n\nParagraph with **strong** and [link](https://example.com).\n\n- item one\n- item two";
        let storage = convert_text(source, BodyFormat::Markdown, BodyFormat::Storage)
            .expect("markdown should convert to storage");
        let markdown = convert_text(&storage, BodyFormat::Storage, BodyFormat::Markdown)
            .expect("storage should convert back to markdown");

        assert!(markdown.contains("# Title"));
        assert!(markdown.contains("**strong**"));
        assert!(markdown.contains("[link](https://example.com)"));
        assert!(markdown.contains("- item one"));
    }

    #[test]
    fn storage_to_text_works_for_simple_document() {
        let text = convert_text(
            "<h1>Title</h1><p>Hello <strong>world</strong></p>",
            BodyFormat::Storage,
            BodyFormat::Text,
        )
        .expect("storage should convert to text");

        assert!(text.contains("Title"));
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn unified_patch_applies_successfully() {
        let base = "Hello\nWorld\n";
        let patch = "--- original\n+++ updated\n@@ -1,2 +1,2 @@\n Hello\n-World\n+Confluence\n";
        let updated = apply_unified_patch(base, patch).expect("patch should apply");
        assert_eq!(updated, "Hello\nConfluence\n");
    }

    #[test]
    fn exports_managed_bundle() {
        let dir = tempdir().expect("tempdir should be created");
        let metadata = build_bundle_metadata(Some(123), Some("Design".to_owned()), Some(7), "body");
        export_managed_bundle(dir.path(), &metadata, "# Design").expect("bundle should export");

        let markdown =
            fs::read_to_string(dir.path().join("page.md")).expect("markdown file should exist");
        let metadata_json = fs::read_to_string(dir.path().join(".confluence").join("page.json"))
            .expect("metadata file should exist");

        assert_eq!(markdown, "# Design");
        assert!(metadata_json.contains("\"page_id\": 123"));
    }
}
