use crate::application::vfs::{DirEntry, NodeCapability, NodeKind, NodeStat};
use crate::NodeHandle;

pub enum ListingStyle {
    Simple,
    Long,
}

pub fn render_listing(entries: &[DirEntry], style: ListingStyle) -> String {
    match style {
        ListingStyle::Simple => render_listing_columns(entries),
        ListingStyle::Long => {
            let mut out = String::new();
            for entry in entries {
                out.push_str(&render_entry_long(entry));
                out.push('\n');
            }
            out
        }
    }
}

pub fn render_file(path: &str, handle: &NodeHandle, stat: &NodeStat) -> String {
    let id = match handle {
        NodeHandle::Root => None,
        NodeHandle::Space(space) => Some(space.id.to_string()),
        NodeHandle::Page(page) => Some(page.id.to_string()),
    };
    let caps = render_capabilities(&stat.capabilities);
    let mut lines = vec![
        format!("path: {path}"),
        format!("kind: {}", render_kind(stat.kind)),
        format!("caps: {caps}"),
        format!("name: {}", stat.name),
    ];
    if let Some(id) = id {
        lines.push(format!("id: {id}"));
    }
    if let Some(has_children) = stat.has_children {
        lines.push(format!("has_children: {has_children}"));
    }
    format!("{}\n", lines.join("\n"))
}

fn render_entry(entry: &DirEntry) -> String {
    match &entry.handle {
        NodeHandle::Space(space) => format!("{}/", space.key),
        NodeHandle::Page(page) => {
            let suffix = if entry.stat.has_children == Some(true) {
                "/"
            } else {
                ""
            };
            format!("{}{}", page.title, suffix)
        }
        NodeHandle::Root => "/".to_owned(),
    }
}

fn render_listing_columns(entries: &[DirEntry]) -> String {
    let names = entries.iter().map(render_entry).collect::<Vec<_>>();
    if names.is_empty() {
        return String::new();
    }

    let terminal_width = 100usize;
    let column_width = names.iter().map(|name| name.len()).max().unwrap_or(0) + 2;
    if column_width >= terminal_width / 2 {
        return format!("{}\n", names.join("\n"));
    }

    let columns = (terminal_width / column_width).max(1);
    let rows = names.len().div_ceil(columns);
    let mut lines = Vec::new();
    for row in 0..rows {
        let mut line = String::new();
        for column in 0..columns {
            let index = column * rows + row;
            if let Some(name) = names.get(index) {
                if column + 1 == columns || index + rows >= names.len() {
                    line.push_str(name);
                } else {
                    line.push_str(&format!("{name:<width$}", width = column_width));
                }
            }
        }
        lines.push(line.trim_end().to_owned());
    }

    format!("{}\n", lines.join("\n"))
}

fn render_entry_long(entry: &DirEntry) -> String {
    match &entry.handle {
        NodeHandle::Space(space) => format!(
            "{:<5} {:<18} {:<10} {}/  {}",
            render_kind(entry.stat.kind),
            render_capabilities(&entry.stat.capabilities),
            space.id,
            space.key,
            space.name
        ),
        NodeHandle::Page(page) => format!(
            "{:<5} {:<18} {:<10} {}",
            render_kind(entry.stat.kind),
            render_capabilities(&entry.stat.capabilities),
            page.id,
            page.title
        ),
        NodeHandle::Root => "root  list,traverse      -          /".to_owned(),
    }
}

fn render_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Root => "root",
        NodeKind::Space => "space",
        NodeKind::Page => "page",
    }
}

fn render_capabilities(capabilities: &[NodeCapability]) -> String {
    capabilities
        .iter()
        .map(|capability| match capability {
            NodeCapability::Read => "read",
            NodeCapability::List => "list",
            NodeCapability::Traverse => "traverse",
            NodeCapability::Search => "search",
            NodeCapability::Create => "create",
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::{render_listing, ListingStyle};
    use crate::{DirEntry, NodeCapability, NodeHandle, NodeKind, NodeStat, PageNode};

    #[test]
    fn simple_listing_uses_columns_for_short_entries() {
        let entries = vec![page("Alpha"), page("Beta"), page("Gamma"), page("Delta")];
        let rendered = render_listing(&entries, ListingStyle::Simple);
        assert!(rendered.lines().count() < entries.len());
    }

    fn page(title: &str) -> DirEntry {
        DirEntry {
            name: title.to_owned(),
            handle: NodeHandle::Page(PageNode {
                id: 1,
                title: title.to_owned(),
                space_id: Some("100".to_owned()),
            }),
            stat: NodeStat {
                kind: NodeKind::Page,
                name: title.to_owned(),
                capabilities: vec![
                    NodeCapability::Read,
                    NodeCapability::List,
                    NodeCapability::Traverse,
                ],
                has_children: None,
            },
        }
    }
}
