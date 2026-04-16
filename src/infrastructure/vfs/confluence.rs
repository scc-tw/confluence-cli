use crate::application::models::{PageSummary, SpaceSummary};
use crate::application::vfs::{
    DirEntry, NodeHandle, NodeKind, NodeStat, PageNode, SpaceNode, VirtualFileSystem,
};
use crate::domain::{BodyFormat, PageId, PageRef};
use crate::support::{ConfluenceCliError, Result};
use crate::PagesApi;

#[derive(Debug, Clone)]
pub struct ConfluenceVfs<A> {
    api: A,
}

impl<A> ConfluenceVfs<A> {
    pub fn new(api: A) -> Self {
        Self { api }
    }
}

impl<A: PagesApi> VirtualFileSystem for ConfluenceVfs<A> {
    fn stat(&self, handle: &NodeHandle) -> Result<NodeStat> {
        match handle {
            NodeHandle::Root => Ok(NodeStat {
                kind: NodeKind::Root,
                name: "/".to_owned(),
                readable: false,
                listable: true,
                has_children: None,
            }),
            NodeHandle::Space(space) => Ok(NodeStat {
                kind: NodeKind::Space,
                name: space.key.clone(),
                readable: false,
                listable: true,
                has_children: None,
            }),
            NodeHandle::Page(page) => Ok(NodeStat {
                kind: NodeKind::Page,
                name: page.title.clone(),
                readable: true,
                listable: true,
                has_children: None,
            }),
        }
    }

    fn read_dir(&self, handle: &NodeHandle) -> Result<Vec<DirEntry>> {
        match handle {
            NodeHandle::Root => self
                .api
                .list_spaces()?
                .into_iter()
                .map(|space| Ok(self.space_entry(space)))
                .collect(),
            NodeHandle::Space(space) => {
                let Some(homepage_id) = space.homepage_id else {
                    return Ok(Vec::new());
                };
                self.page_entries(&PageRef::Id(PageId::new(homepage_id)))
            }
            NodeHandle::Page(page) => self.page_entries(&PageRef::Id(PageId::new(page.id))),
        }
    }

    fn open_child(&self, parent: &NodeHandle, name: &str) -> Result<NodeHandle> {
        let entries = self.read_dir(parent)?;
        if entries.is_empty() {
            return Err(ConfluenceCliError::Config(format!(
                "'{name}' not found under {}",
                self.display_name(parent)
            )));
        }

        if let Some(entry) = entries
            .iter()
            .find(|entry| self.matches_node(entry, name, true))
        {
            return Ok(entry.handle.clone());
        }

        let name_matches: Vec<_> = entries
            .iter()
            .filter(|entry| self.matches_node(entry, name, false))
            .collect();
        match name_matches.as_slice() {
            [entry] => Ok(entry.handle.clone()),
            [] => Err(ConfluenceCliError::Config(format!(
                "'{name}' not found under {}",
                self.display_name(parent)
            ))),
            _ => Err(ConfluenceCliError::Config(format!(
                "'{name}' is ambiguous under {}; use an id instead",
                self.display_name(parent)
            ))),
        }
    }

    fn read(&self, handle: &NodeHandle) -> Result<String> {
        match handle {
            NodeHandle::Page(page) => Ok(self
                .api
                .read_page(&PageRef::Id(PageId::new(page.id)), BodyFormat::Storage)?
                .content),
            NodeHandle::Root | NodeHandle::Space(_) => Err(ConfluenceCliError::Config(format!(
                "{} is not readable",
                self.display_name(handle)
            ))),
        }
    }
}

impl<A: PagesApi> ConfluenceVfs<A> {
    fn page_entries(&self, page: &PageRef) -> Result<Vec<DirEntry>> {
        self.api
            .list_child_pages(page)?
            .into_iter()
            .map(|summary| self.page_entry(summary))
            .collect()
    }

    fn page_entry(&self, summary: PageSummary) -> Result<DirEntry> {
        let title = summary.title;
        let id = summary.id;
        let space_id = summary.space_id;
        Ok(DirEntry {
            name: title.clone(),
            handle: NodeHandle::Page(PageNode {
                id,
                title: title.clone(),
                space_id,
            }),
            stat: NodeStat {
                kind: NodeKind::Page,
                name: title,
                readable: true,
                listable: true,
                has_children: None,
            },
        })
    }

    fn space_entry(&self, summary: SpaceSummary) -> DirEntry {
        let display_name = summary.key.clone();
        DirEntry {
            name: display_name.clone(),
            handle: NodeHandle::Space(SpaceNode {
                id: summary.id,
                key: summary.key,
                name: summary.name,
                homepage_id: summary.homepage_id,
            }),
            stat: NodeStat {
                kind: NodeKind::Space,
                name: display_name,
                readable: false,
                listable: true,
                has_children: None,
            },
        }
    }

    fn matches_node(&self, entry: &DirEntry, name: &str, exact_id: bool) -> bool {
        match &entry.handle {
            NodeHandle::Space(space) => {
                space.id == name
                    || space.key.eq_ignore_ascii_case(name)
                    || (!exact_id && space.name.eq_ignore_ascii_case(name))
            }
            NodeHandle::Page(page) => {
                page.id.to_string() == name || (!exact_id && page.title.eq_ignore_ascii_case(name))
            }
            NodeHandle::Root => false,
        }
    }

    fn display_name(&self, handle: &NodeHandle) -> String {
        match handle {
            NodeHandle::Root => "/".to_owned(),
            NodeHandle::Space(space) => format!("/{}", space.key),
            NodeHandle::Page(page) => page.title.clone(),
        }
    }
}
