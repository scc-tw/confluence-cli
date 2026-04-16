use crate::application::vfs::{NodeHandle, VirtualFileSystem};
use crate::support::Result;
use crate::GlobalArgs;

use super::super::bootstrap::load_runtime_and_vfs;

pub struct ShellState {
    global: GlobalArgs,
    vfs: Box<dyn VirtualFileSystem>,
    lineage: Vec<NodeHandle>,
}

impl ShellState {
    pub fn new(global: GlobalArgs, vfs: Box<dyn VirtualFileSystem>) -> Self {
        Self {
            global,
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
        let (_, vfs) = load_runtime_and_vfs(&self.global)?;
        self.vfs = vfs;
        self.lineage = vec![self.vfs.root()];
        Ok(())
    }

    pub fn current_page_ref(&self) -> Option<crate::domain::PageRef> {
        self.current().as_page_ref()
    }

    pub fn current_space(&self) -> Option<crate::SpaceNode> {
        self.lineage.iter().find_map(NodeHandle::as_space).cloned()
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
