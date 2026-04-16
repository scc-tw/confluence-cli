use serde::Serialize;

use crate::application::models::PageContentKind;
use crate::domain::{PageId, PageRef};
use crate::support::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpaceNode {
    pub id: String,
    pub key: String,
    pub name: String,
    pub homepage_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageNode {
    pub id: u64,
    pub title: String,
    pub space_id: Option<String>,
    pub content_kind: PageContentKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum NodeHandle {
    Root,
    Space(SpaceNode),
    Page(PageNode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NodeKind {
    Root,
    Space,
    Page,
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum NodeCapability {
    Read,
    List,
    Traverse,
    Search,
    Create,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeStat {
    pub kind: NodeKind,
    pub name: String,
    pub capabilities: Vec<NodeCapability>,
    pub has_children: Option<bool>,
}

impl NodeStat {
    pub fn supports(&self, capability: NodeCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub handle: NodeHandle,
    pub stat: NodeStat,
}

pub trait VirtualFileSystem {
    fn root(&self) -> NodeHandle {
        NodeHandle::Root
    }

    fn stat(&self, handle: &NodeHandle) -> Result<NodeStat>;
    fn read_dir(&self, handle: &NodeHandle) -> Result<Vec<DirEntry>>;
    fn open_child(&self, parent: &NodeHandle, name: &str) -> Result<NodeHandle>;
    fn read(&self, handle: &NodeHandle) -> Result<String>;
}

impl NodeHandle {
    pub fn as_page_ref(&self) -> Option<PageRef> {
        match self {
            NodeHandle::Page(page) => Some(PageRef::Id(PageId::new(page.id))),
            _ => None,
        }
    }

    pub fn as_space(&self) -> Option<&SpaceNode> {
        match self {
            NodeHandle::Space(space) => Some(space),
            _ => None,
        }
    }
}
