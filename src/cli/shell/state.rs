use crate::GlobalArgs;
use crate::application::runtime::{ResolvedProfile, RuntimeContext, ensure_writable};
use crate::application::vfs::{NodeHandle, VirtualFileSystem};
use crate::support::Result;

use super::super::bootstrap::load_runtime_and_vfs;

pub struct ShellState {
    global: GlobalArgs,
    runtime: RuntimeContext,
    resolved_profile: Option<ResolvedProfile>,
    vfs: Box<dyn VirtualFileSystem>,
    lineage: Vec<NodeHandle>,
}

impl ShellState {
    pub fn new(
        global: GlobalArgs,
        runtime: RuntimeContext,
        vfs: Box<dyn VirtualFileSystem>,
    ) -> Self {
        Self {
            global,
            runtime: runtime.clone(),
            resolved_profile: runtime.runtime_config.resolved_profile,
            lineage: vec![vfs.root()],
            vfs,
        }
    }

    pub fn prompt(&self) -> String {
        if let Some(profile) = &self.global.profile {
            format!("confluence({profile}){}> ", self.cwd_display())
        } else {
            format!("confluence{}> ", self.cwd_display())
        }
    }

    pub fn cwd_display(&self) -> String {
        if self.lineage.len() == 1 {
            return "/".to_owned();
        }
        let segments = self
            .lineage
            .iter()
            .skip(1)
            .map(|handle| match handle {
                NodeHandle::Space(space) => space.key.clone(),
                NodeHandle::Page(page) => page.title.clone(),
                NodeHandle::Root => unreachable!("root only appears as the first path element"),
            })
            .collect::<Vec<_>>();
        format!("/{}", segments.join("/"))
    }

    pub fn current(&self) -> &NodeHandle {
        self.lineage
            .last()
            .expect("shell lineage always contains the root")
    }

    pub fn vfs(&self) -> &dyn VirtualFileSystem {
        self.vfs.as_ref()
    }

    pub fn global(&self) -> &GlobalArgs {
        &self.global
    }

    pub fn resolved_profile(&self) -> Option<&ResolvedProfile> {
        self.resolved_profile.as_ref()
    }

    pub fn ensure_writable(&self) -> Result<()> {
        ensure_writable(&self.runtime)
    }

    pub fn resolve_listing_target(&self, target: Option<&str>) -> Result<NodeHandle> {
        match target {
            None => Ok(self.current().clone()),
            Some(target) => Ok(self
                .resolve_lineage(target)?
                .last()
                .cloned()
                .expect("resolved lineage always contains at least the root")),
        }
    }

    pub fn resolve_target_lineage(&self, target: Option<&str>) -> Result<Vec<NodeHandle>> {
        match target {
            None => Ok(self.lineage.clone()),
            Some(target) => self.resolve_lineage(target),
        }
    }

    pub fn render_lineage(&self, lineage: &[NodeHandle]) -> String {
        if lineage.len() <= 1 {
            return "/".to_owned();
        }

        let segments = lineage
            .iter()
            .skip(1)
            .map(|handle| match handle {
                NodeHandle::Space(space) => space.key.clone(),
                NodeHandle::Page(page) => page.title.clone(),
                NodeHandle::Root => unreachable!("root only appears as the first path element"),
            })
            .collect::<Vec<_>>();
        format!("/{}", segments.join("/"))
    }

    pub fn change_directory(&mut self, target: &str) -> Result<()> {
        match target {
            "/" => {
                self.lineage.truncate(1);
                Ok(())
            }
            ".." => {
                if self.lineage.len() > 1 {
                    self.lineage.pop();
                }
                Ok(())
            }
            _ => {
                self.lineage = self.resolve_lineage(target)?;
                Ok(())
            }
        }
    }

    pub fn use_profile(&mut self, profile: String) -> Result<()> {
        self.global.profile = Some(profile);
        let (runtime, vfs) = load_runtime_and_vfs(&self.global)?;
        self.runtime = runtime.clone();
        self.resolved_profile = runtime.runtime_config.resolved_profile;
        self.vfs = vfs;
        self.lineage = vec![self.vfs.root()];
        Ok(())
    }

    pub fn resolve_parent_for_create(&self, path: &str) -> Result<(Vec<NodeHandle>, String)> {
        let trimmed = path.trim_end_matches('/');
        if trimmed.is_empty() || trimmed == "/" {
            return Err(crate::ConfluenceCliError::Config(
                "target path must include a leaf name".to_owned(),
            ));
        }

        let mut parts = trimmed.rsplitn(2, '/');
        let leaf = parts.next().expect("split always yields at least one part");
        if leaf.is_empty() || matches!(leaf, "." | "..") {
            return Err(crate::ConfluenceCliError::Config(
                "target leaf name must be a normal path segment".to_owned(),
            ));
        }
        let parent_path = parts.next().unwrap_or(".");
        Ok((
            self.resolve_target_lineage(Some(parent_path))?,
            leaf.to_owned(),
        ))
    }

    pub fn current_page_ref(&self) -> Option<crate::domain::PageRef> {
        self.current().as_page_ref()
    }

    fn resolve_lineage(&self, path: &str) -> Result<Vec<NodeHandle>> {
        if path == "/" {
            return Ok(vec![self.vfs.root()]);
        }

        let mut lineage = if path.starts_with('/') {
            vec![self.vfs.root()]
        } else {
            self.lineage.clone()
        };

        for segment in path.split('/').filter(|segment| !segment.is_empty()) {
            match segment {
                "." => {}
                ".." => {
                    if lineage.len() > 1 {
                        lineage.pop();
                    }
                }
                _ => {
                    let parent = lineage.last().expect("lineage always has root").clone();
                    let next = self.vfs.open_child(&parent, segment)?;
                    lineage.push(next);
                }
            }
        }

        if lineage.is_empty() {
            lineage.push(self.vfs.root());
        }

        Ok(lineage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtime::{RuntimeConfig, RuntimeProfiles};
    use crate::application::vfs::{DirEntry, NodeCapability, NodeKind, NodeStat, SpaceNode};

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
                        NodeCapability::Create,
                    ],
                    has_children: None,
                },
                NodeHandle::Page(page) => NodeStat {
                    kind: NodeKind::Page,
                    name: page.title.clone(),
                    capabilities: vec![NodeCapability::Read],
                    has_children: None,
                },
            })
        }

        fn read_dir(&self, handle: &NodeHandle) -> Result<Vec<DirEntry>> {
            Ok(match handle {
                NodeHandle::Root => vec![DirEntry {
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
                            NodeCapability::Create,
                        ],
                        has_children: None,
                    },
                }],
                _ => Vec::new(),
            })
        }

        fn open_child(&self, parent: &NodeHandle, name: &str) -> Result<NodeHandle> {
            self.read_dir(parent)?
                .into_iter()
                .find(|entry| {
                    entry.name == name
                        || matches!(&entry.handle, NodeHandle::Space(space) if space.key == name)
                })
                .map(|entry| entry.handle)
                .ok_or_else(|| crate::ConfluenceCliError::Config(format!("'{name}' not found")))
        }

        fn read(&self, _handle: &NodeHandle) -> Result<String> {
            Ok(String::new())
        }
    }

    #[test]
    fn resolve_parent_for_create_splits_leaf_from_path() {
        let runtime = RuntimeContext {
            runtime_config: RuntimeConfig {
                profiles: RuntimeProfiles {
                    active_profile: Some("work".to_owned()),
                    profiles: vec!["work".to_owned()],
                },
                resolved_profile: None,
            },
        };
        let state = ShellState::new(
            GlobalArgs {
                config_path: None,
                profile: Some("work".to_owned()),
                output: crate::OutputFormat::Human,
            },
            runtime,
            Box::new(FakeVfs),
        );

        let (lineage, leaf) = state.resolve_parent_for_create("ALPHA/New Page").unwrap();
        assert_eq!(leaf, "New Page");
        assert_eq!(state.render_lineage(&lineage), "/ALPHA");
    }

    #[test]
    fn ensure_writable_rejects_read_only_runtime() {
        let runtime = RuntimeContext {
            runtime_config: RuntimeConfig {
                profiles: RuntimeProfiles {
                    active_profile: Some("work".to_owned()),
                    profiles: vec!["work".to_owned()],
                },
                resolved_profile: Some(ResolvedProfile {
                    id: "p-1".to_owned(),
                    name: Some("work".to_owned()),
                    domain: "example.atlassian.net".to_owned(),
                    protocol: "https".to_owned(),
                    api_path: "/wiki/rest/api".to_owned(),
                    auth_type: crate::AuthKind::Bearer,
                    email: None,
                    username: None,
                    api_token: Some("token".to_owned()),
                    password: None,
                    read_only: true,
                }),
            },
        };
        let state = ShellState::new(
            GlobalArgs {
                config_path: None,
                profile: Some("work".to_owned()),
                output: crate::OutputFormat::Human,
            },
            runtime,
            Box::new(FakeVfs),
        );

        assert!(state.ensure_writable().is_err());
    }
}
