use crate::application::models::{
    CreateFolderRequest, CreatePageRequest, MovePageRequest, PageContentKind, PageSummary,
    SpaceSummary,
};
use crate::application::vfs::{
    DirEntry, NodeCapability, NodeHandle, NodeKind, NodeStat, PageNode, SpaceNode,
    VirtualFileSystem,
};
use crate::domain::{BodyFormat, DeleteMode, MoveTarget, PageId, PageRef};
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
                capabilities: vec![NodeCapability::List, NodeCapability::Traverse],
                has_children: None,
            }),
            NodeHandle::Space(space) => Ok(NodeStat {
                kind: NodeKind::Space,
                name: space.key.clone(),
                capabilities: vec![
                    NodeCapability::List,
                    NodeCapability::Traverse,
                    NodeCapability::Search,
                    NodeCapability::Create,
                ],
                has_children: None,
            }),
            NodeHandle::Page(page) => Ok(NodeStat {
                kind: node_kind_for(page.content_kind.clone()),
                name: page.title.clone(),
                capabilities: page_capabilities(&page.content_kind),
                has_children: if matches!(page.content_kind, PageContentKind::Folder) {
                    Some(true)
                } else {
                    None
                },
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
                self.page_entries(
                    &PageRef::Id(PageId::new(homepage_id)),
                    PageContentKind::Page,
                )
            }
            NodeHandle::Page(page) => self.page_entries(
                &PageRef::Id(PageId::new(page.id)),
                page.content_kind.clone(),
            ),
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

    fn create_child(&self, parent: &NodeHandle, name: &str, kind: NodeKind) -> Result<NodeHandle> {
        if name.trim().is_empty() {
            return Err(ConfluenceCliError::Config(
                "node name must not be empty".to_owned(),
            ));
        }

        let (space_id, parent_id) = self.parent_context(parent)?;
        let node = match kind {
            NodeKind::Folder => self.api.create_folder(CreateFolderRequest {
                title: name.to_owned(),
                space_id,
                parent_id,
            })?,
            NodeKind::Page => self.api.create_page(CreatePageRequest {
                title: name.to_owned(),
                storage_body: "<p></p>".to_owned(),
                space_id,
                parent_id,
            })?,
            NodeKind::Root | NodeKind::Space => {
                return Err(ConfluenceCliError::Config(
                    "create_child supports page or folder kinds only".to_owned(),
                ))
            }
        };
        self.page_entry(node).map(|entry| entry.handle)
    }

    fn remove_node(&self, handle: &NodeHandle) -> Result<()> {
        let NodeHandle::Page(page) = handle else {
            return Err(ConfluenceCliError::Config(
                "only pages or folders can be removed".to_owned(),
            ));
        };
        match page.content_kind {
            PageContentKind::Folder => self.api.delete_folder(&PageRef::Id(PageId::new(page.id))),
            PageContentKind::Page => self
                .api
                .delete_page(&PageRef::Id(PageId::new(page.id)), DeleteMode::Archive),
        }
    }

    fn move_node(
        &self,
        handle: &NodeHandle,
        new_parent: &NodeHandle,
        new_name: Option<&str>,
    ) -> Result<NodeHandle> {
        let NodeHandle::Page(page) = handle else {
            return Err(ConfluenceCliError::Config(
                "only pages or folders can be moved".to_owned(),
            ));
        };
        let target_parent = self.parent_page_ref(new_parent)?;
        let moved = self.api.move_page(MovePageRequest {
            page: PageRef::Id(PageId::new(page.id)),
            target: MoveTarget::Parent(target_parent),
            title: new_name.map(str::to_owned),
        })?;
        self.page_entry(moved).map(|entry| entry.handle)
    }

    fn copy_node(
        &self,
        handle: &NodeHandle,
        new_parent: &NodeHandle,
        new_name: Option<&str>,
    ) -> Result<NodeHandle> {
        let NodeHandle::Page(page) = handle else {
            return Err(ConfluenceCliError::Config(
                "only pages or folders can be copied".to_owned(),
            ));
        };
        let body = self
            .api
            .read_page(&PageRef::Id(PageId::new(page.id)), BodyFormat::Storage)?;
        let (space_id, parent_id) = self.parent_context(new_parent)?;
        let copied = self.api.create_page(CreatePageRequest {
            title: new_name.unwrap_or(&page.title).to_owned(),
            storage_body: body.content,
            space_id,
            parent_id,
        })?;
        self.page_entry(copied).map(|entry| entry.handle)
    }
}

impl<A: PagesApi> ConfluenceVfs<A> {
    fn page_entries(&self, page: &PageRef, parent_kind: PageContentKind) -> Result<Vec<DirEntry>> {
        self.api
            .list_child_content(page, parent_kind)?
            .into_iter()
            .map(|summary| self.page_entry(summary))
            .collect()
    }

    fn page_entry(&self, summary: PageSummary) -> Result<DirEntry> {
        let title = summary.title;
        let id = summary.id;
        let space_id = summary.space_id;
        let content_kind = summary.content_kind;
        Ok(DirEntry {
            name: title.clone(),
            handle: NodeHandle::Page(PageNode {
                id,
                title: title.clone(),
                space_id,
                content_kind: content_kind.clone(),
            }),
            stat: NodeStat {
                kind: node_kind_for(content_kind.clone()),
                name: title,
                capabilities: page_capabilities(&content_kind),
                has_children: if matches!(content_kind, PageContentKind::Folder) {
                    Some(true)
                } else {
                    None
                },
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
                capabilities: vec![
                    NodeCapability::List,
                    NodeCapability::Traverse,
                    NodeCapability::Search,
                    NodeCapability::Create,
                ],
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

fn node_kind_for(content_kind: PageContentKind) -> NodeKind {
    match content_kind {
        PageContentKind::Page => NodeKind::Page,
        PageContentKind::Folder => NodeKind::Folder,
    }
}

fn page_capabilities(content_kind: &PageContentKind) -> Vec<NodeCapability> {
    match content_kind {
        PageContentKind::Page => vec![
            NodeCapability::Read,
            NodeCapability::List,
            NodeCapability::Traverse,
            NodeCapability::Create,
            NodeCapability::Delete,
            NodeCapability::Move,
            NodeCapability::Copy,
        ],
        PageContentKind::Folder => vec![
            NodeCapability::List,
            NodeCapability::Traverse,
            NodeCapability::Search,
            NodeCapability::Create,
            NodeCapability::Delete,
            NodeCapability::Move,
            NodeCapability::Copy,
        ],
    }
}

impl<A: PagesApi> ConfluenceVfs<A> {
    fn parent_context(&self, parent: &NodeHandle) -> Result<(String, Option<PageId>)> {
        match parent {
            NodeHandle::Space(space) => Ok((space.id.clone(), space.homepage_id.map(PageId::new))),
            NodeHandle::Page(page) => Ok((
                page.space_id.clone().ok_or_else(|| {
                    ConfluenceCliError::Config("page parent did not expose a space id".to_owned())
                })?,
                Some(PageId::new(page.id)),
            )),
            NodeHandle::Root => Err(ConfluenceCliError::Config(
                "root is not a writable parent".to_owned(),
            )),
        }
    }

    fn parent_page_ref(&self, parent: &NodeHandle) -> Result<PageRef> {
        match parent {
            NodeHandle::Space(space) => space
                .homepage_id
                .map(PageId::new)
                .map(PageRef::Id)
                .ok_or_else(|| {
                    ConfluenceCliError::Config("space does not expose a homepage id".to_owned())
                }),
            NodeHandle::Page(page) => Ok(PageRef::Id(PageId::new(page.id))),
            NodeHandle::Root => Err(ConfluenceCliError::Config(
                "root is not a writable parent".to_owned(),
            )),
        }
    }
}
