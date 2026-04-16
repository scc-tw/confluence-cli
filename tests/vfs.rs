use confluence_cli::{
    ConfluenceCliError, DirEntry, NodeCapability, NodeHandle, NodeKind, NodeStat, PageNode, Result,
    SpaceNode, VirtualFileSystem,
};

struct FakeVfs;

impl VirtualFileSystem for FakeVfs {
    fn stat(&self, handle: &NodeHandle) -> Result<NodeStat> {
        Ok(match handle {
            NodeHandle::Root => NodeStat {
                kind: NodeKind::Root,
                name: "/".to_owned(),
                capabilities: vec![NodeCapability::List, NodeCapability::Traverse],
                has_children: None,
            },
            NodeHandle::Space(space) => NodeStat {
                kind: NodeKind::Space,
                name: space.key.clone(),
                capabilities: vec![
                    NodeCapability::List,
                    NodeCapability::Traverse,
                    NodeCapability::Search,
                    NodeCapability::Create,
                ],
                has_children: None,
            },
            NodeHandle::Page(page) => NodeStat {
                kind: NodeKind::Page,
                name: page.title.clone(),
                capabilities: vec![
                    NodeCapability::Read,
                    NodeCapability::List,
                    NodeCapability::Traverse,
                ],
                has_children: None,
            },
        })
    }

    fn read_dir(&self, handle: &NodeHandle) -> Result<Vec<DirEntry>> {
        match handle {
            NodeHandle::Root => Ok(vec![DirEntry {
                name: "ALPHA".to_owned(),
                handle: NodeHandle::Space(SpaceNode {
                    id: "100".to_owned(),
                    key: "ALPHA".to_owned(),
                    name: "Workspace Alpha".to_owned(),
                    homepage_id: Some(1),
                }),
                stat: NodeStat {
                    kind: NodeKind::Space,
                    name: "ALPHA".to_owned(),
                    capabilities: vec![
                        NodeCapability::List,
                        NodeCapability::Traverse,
                        NodeCapability::Search,
                        NodeCapability::Create,
                    ],
                    has_children: None,
                },
            }]),
            NodeHandle::Space(space) if space.key == "ALPHA" => Ok(vec![
                DirEntry {
                    name: "Notebook".to_owned(),
                    handle: NodeHandle::Page(PageNode {
                        id: 2,
                        title: "Notebook".to_owned(),
                        space_id: Some("100".to_owned()),
                    }),
                    stat: NodeStat {
                        kind: NodeKind::Page,
                        name: "Notebook".to_owned(),
                        capabilities: vec![
                            NodeCapability::Read,
                            NodeCapability::List,
                            NodeCapability::Traverse,
                        ],
                        has_children: None,
                    },
                },
                DirEntry {
                    name: "Notebook".to_owned(),
                    handle: NodeHandle::Page(PageNode {
                        id: 3,
                        title: "Notebook".to_owned(),
                        space_id: Some("100".to_owned()),
                    }),
                    stat: NodeStat {
                        kind: NodeKind::Page,
                        name: "Notebook".to_owned(),
                        capabilities: vec![
                            NodeCapability::Read,
                            NodeCapability::List,
                            NodeCapability::Traverse,
                        ],
                        has_children: None,
                    },
                },
            ]),
            _ => Ok(Vec::new()),
        }
    }

    fn open_child(&self, parent: &NodeHandle, name: &str) -> Result<NodeHandle> {
        let entries = self.read_dir(parent)?;
        if let Some(entry) = entries.iter().find(|entry| match &entry.handle {
            NodeHandle::Space(space) => space.key == name || space.id == name,
            NodeHandle::Page(page) => page.id.to_string() == name,
            NodeHandle::Root => false,
        }) {
            return Ok(entry.handle.clone());
        }

        let title_matches: Vec<_> = entries
            .iter()
            .filter(|entry| match &entry.handle {
                NodeHandle::Space(space) => space.name == name,
                NodeHandle::Page(page) => page.title == name,
                NodeHandle::Root => false,
            })
            .collect();

        match title_matches.as_slice() {
            [entry] => Ok(entry.handle.clone()),
            [] => Err(ConfluenceCliError::Config(format!(
                "'{name}' not found under /ENG"
            ))),
            _ => Err(ConfluenceCliError::Config(format!(
                "'{name}' is ambiguous under /ENG; use an id instead"
            ))),
        }
    }

    fn read(&self, handle: &NodeHandle) -> Result<String> {
        match handle {
            NodeHandle::Page(page) => Ok(format!("<p>{}</p>", page.title)),
            _ => Err(ConfluenceCliError::Config(
                "node is not readable".to_owned(),
            )),
        }
    }
}

#[test]
fn fake_vfs_root_lists_spaces() {
    let entries = FakeVfs
        .read_dir(&NodeHandle::Root)
        .expect("root should list spaces");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "ALPHA");
}

#[test]
fn fake_vfs_requires_page_id_when_titles_are_ambiguous() {
    let space = NodeHandle::Space(SpaceNode {
        id: "100".to_owned(),
        key: "ALPHA".to_owned(),
        name: "Workspace Alpha".to_owned(),
        homepage_id: Some(1),
    });

    let error = FakeVfs
        .open_child(&space, "Notebook")
        .expect_err("duplicate titles should be ambiguous");

    match error {
        ConfluenceCliError::Config(message) => {
            assert!(message.contains("ambiguous"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn fake_vfs_stat_exposes_capabilities() {
    let page = NodeHandle::Page(PageNode {
        id: 2,
        title: "Notebook".to_owned(),
        space_id: Some("100".to_owned()),
    });

    let stat = FakeVfs.stat(&page).expect("page stat should be available");
    assert!(stat.supports(NodeCapability::Read));
    assert!(stat.supports(NodeCapability::List));
    assert!(stat.supports(NodeCapability::Traverse));
    assert!(!stat.supports(NodeCapability::Create));
}
