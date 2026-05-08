use std::fs;
use structured_agent_workspace::workspace::WorkspaceServer;
use tempfile::TempDir;

const TEST_RS_FIXTURE: &str = include_str!("../fixtures/test.rs");
const TEST_PY_FIXTURE: &str = include_str!("../fixtures/test.py");
const TEST_SH_FIXTURE: &str = include_str!("../fixtures/test.sh");
const TEST_MD_FIXTURE: &str = include_str!("../fixtures/test.md");

pub fn create_test_workspace() -> (TempDir, WorkspaceServer) {
    let temp_dir = TempDir::new().unwrap();
    let workspace_root = temp_dir.path().to_path_buf();

    fs::write(workspace_root.join("test.rs"), TEST_RS_FIXTURE).unwrap();
    fs::write(workspace_root.join("test.py"), TEST_PY_FIXTURE).unwrap();
    fs::write(workspace_root.join("test.sh"), TEST_SH_FIXTURE).unwrap();
    fs::write(workspace_root.join("test.md"), TEST_MD_FIXTURE).unwrap();

    let server = WorkspaceServer::new(workspace_root);
    (temp_dir, server)
}
