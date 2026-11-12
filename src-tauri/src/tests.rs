use crate::{
    app::{plan_view, Hub},
    files::*,
    model::*,
};
use std::{fs, path::Path};

fn hub_at(path: &Path) -> Hub {
    let mut hub = Hub::new();
    hub.storage = Some(path.join("private"));
    hub.home_override = Some(path.join("home"));
    fs::create_dir_all(hub.home().unwrap()).unwrap();
    hub
}

#[test]
fn external_change_prevents_all_writes() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let a = hub.home().unwrap().join(".claude.json");
    let b = hub.home().unwrap().join(".cursor/mcp.json");
    fs::create_dir_all(b.parent().unwrap()).unwrap();
    fs::write(&a, "before a").unwrap();
    fs::write(&b, "before b").unwrap();
    let changes = vec![
        Change {
            path: path_string(&a),
            before: snapshot(&a).unwrap(),
            after: file_snapshot("after a".into()),
            sensitive: false,
        },
        Change {
            path: path_string(&b),
            before: snapshot(&b).unwrap(),
            after: file_snapshot("after b".into()),
            sensitive: false,
        },
    ];
    let view = hub.queue("change".into(), changes, vec![]).unwrap();
    fs::write(&b, "external edit").unwrap();
    assert!(hub.apply(&view.id).err().unwrap().contains("外部修改"));
    assert_eq!(fs::read_to_string(a).unwrap(), "before a");
    assert_eq!(fs::read_to_string(b).unwrap(), "external edit");
}

#[test]
fn apply_keeps_private_preimage_and_restores_files_and_empty_directories() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let target = hub.home().unwrap().join(".agents/skills/demo");
    fs::create_dir_all(target.join("empty")).unwrap();
    fs::write(target.join("SKILL.md"), "before").unwrap();
    let before = snapshot(&target).unwrap();
    let mut after = before.clone();
    after.entries[0].bytes = b"after".to_vec();
    let plan = hub
        .queue(
            "deploy".into(),
            vec![Change {
                path: path_string(&target),
                before: before.clone(),
                after: after.clone(),
                sensitive: false,
            }],
            vec![],
        )
        .unwrap();
    hub.apply(&plan.id).unwrap();
    assert!(snapshot(&target).unwrap() == after);
    let backup = hub
        .root()
        .unwrap()
        .join("backups")
        .join(format!("{}.json", plan.id));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let saved: Plan = serde_json::from_slice(&fs::read(backup).unwrap()).unwrap();
    assert!(saved.changes[0].before == before);
    let undo = hub
        .queue(
            "undo".into(),
            vec![Change {
                path: path_string(&target),
                before: after,
                after: before.clone(),
                sensitive: false,
            }],
            vec![],
        )
        .unwrap();
    hub.apply(&undo.id).unwrap();
    assert!(snapshot(&target).unwrap() == before);
    assert!(target.join("empty").is_dir());
}

#[test]
fn path_traversal_and_symlinks_are_rejected() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(dir.path(), &link).unwrap();
    assert!(snapshot(&link).is_err());
    assert!(ensure_no_symlink_ancestors(&link.join("child")).is_err());
    assert!(checked_path("/tmp/../etc").is_err());
    let invalid = Snapshot {
        exists: true,
        directory: true,
        entries: vec![Entry {
            path: "../escape".into(),
            bytes: vec![],
            executable: false,
        }],
        directories: vec![],
    };
    assert!(validate_snapshot(&invalid).is_err());
}

#[test]
fn source_excludes_git_but_backup_preserves_it() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    fs::write(dir.path().join(".git/config"), "important").unwrap();
    assert_eq!(source_snapshot(dir.path()).unwrap().entries.len(), 0);
    assert_eq!(snapshot(dir.path()).unwrap().entries.len(), 1);
}

#[test]
fn sensitive_previews_never_include_values() {
    let plan=Plan{undo_of:None,id:"1".into(),title:"mcp".into(),created_at:"0".into(),notes:vec![],changes:vec![Change{path:"/tmp/config".into(),before:file_snapshot(r#"{"mcpServers":{"private":{"headers":{"Authorization":"secret-before"},"args":["token-in-args"]}}}"#.into()),after:file_snapshot(r#"{"mcpServers":{"private":{"headers":{"Authorization":"secret-after"},"args":["new-token"]}}}"#.into()),sensitive:true}]};
    let view = plan_view(&plan);
    let output = serde_json::to_string(&view).unwrap();
    for secret in [
        "secret-before",
        "secret-after",
        "token-in-args",
        "new-token",
    ] {
        assert!(!output.contains(secret));
    }
    assert!(!view.changes[0].diff.is_empty());
}

#[test]
fn observations_do_not_invalidate_library_undo() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let s = Settings::default();
    hub.save(&s).unwrap();
    let before = snapshot(&hub.settings_path().unwrap()).unwrap();
    atomic_write(
        &hub.root().unwrap().join("observations.json"),
        br#"{"resource":"fingerprint"}"#,
        false,
    )
    .unwrap();
    assert!(snapshot(&hub.settings_path().unwrap()).unwrap() == before);
    assert_eq!(
        hub.settings()
            .unwrap()
            .observations
            .get("resource")
            .unwrap(),
        "fingerprint"
    );
}

#[test]
fn mcp_edits_preserve_credentials_unknown_fields_and_toml_comments() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let project = dir.path().join("project");
    fs::create_dir(&project).unwrap();
    let mut settings = Settings::default();
    settings.projects.push(path_string(&project));
    hub.save(&settings).unwrap();
    for agent in ["claude", "codex", "cursor", "antigravity"] {
        let target = Target {
            agent: agent.into(),
            project: Some(path_string(&project)),
        };
        let p = hub.target(&target, false).unwrap();
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let native = if agent == "codex" {
            "# preserve comment\nother_setting = true\n[mcp_servers.demo]\ncommand = 'old'\ncustom = 42\n[mcp_servers.demo.env]\nAPI_TOKEN = 'kept-secret'\n".into()
        } else {
            serde_json::json!({"unknown":true,"mcpServers":{"demo":{"command":"old","custom":42,"env":{"API_TOKEN":"kept-secret"}}}}).to_string()
        };
        fs::write(&p, native).unwrap();
        let view = crate::commands::mcp_plan(
            &hub,
            "demo",
            Some(serde_json::json!({"command":"new","args":["--version"]})),
            vec![target],
        )
        .unwrap();
        assert!(!serde_json::to_string(&view)
            .unwrap()
            .contains("kept-secret"));
        hub.apply(&view.id).unwrap();
        let v = crate::scan::parse_config(&p, agent).unwrap();
        let entry = &crate::scan::servers(&v, agent).unwrap()["demo"];
        assert_eq!(entry["env"]["API_TOKEN"], "kept-secret");
        assert_eq!(entry["custom"], 42);
        assert_eq!(entry["command"], "new");
        if agent == "codex" {
            assert!(fs::read_to_string(p)
                .unwrap()
                .contains("# preserve comment"));
        }
    }
}

#[test]
fn malformed_bundle_paths_cannot_be_written() {
    let s = Snapshot {
        exists: true,
        directory: true,
        entries: vec![
            Entry {
                path: "a".into(),
                bytes: vec![],
                executable: false,
            },
            Entry {
                path: "a/child".into(),
                bytes: vec![],
                executable: false,
            },
        ],
        directories: vec![],
    };
    assert!(validate_snapshot(&s).is_err());
}

#[test]
fn unapproved_paths_and_cross_purpose_grants_are_denied() {
    use crate::access::PathPurpose;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let source = dir.path().join("source");
    fs::create_dir(&source).unwrap();
    assert!(hub.require_grant(PathPurpose::Source, &source).is_err());
    hub.grant(PathPurpose::Source, source.clone()).unwrap();
    assert!(hub.require_grant(PathPurpose::Source, &source).is_ok());
    assert!(hub.require_grant(PathPurpose::Export, &source).is_err());
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, source.join("escape")).unwrap();
    assert!(hub
        .require_grant(PathPurpose::Source, &source.join("escape/secret"))
        .is_err());
    assert!(hub.require_grant(PathPurpose::Source, &outside).is_err());
}

#[test]
fn plan_cannot_write_outside_agent_paths_even_with_a_valid_plan_id() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let path = dir.path().join("unrelated.txt");
    fs::write(&path, "user content").unwrap();
    let view = hub
        .queue(
            "unauthorized target".into(),
            vec![Change {
                path: path_string(&path),
                before: snapshot(&path).unwrap(),
                after: file_snapshot("replacement".into()),
                sensitive: false,
            }],
            vec![],
        )
        .unwrap();
    assert!(hub.apply(&view.id).err().unwrap().contains("超出"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "user content");
}

#[test]
fn library_metadata_cannot_redirect_reads_and_remote_http_requires_tls() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let skill = Skill {
        id: "a".repeat(64),
        name: "demo".into(),
        description: String::new(),
        source: String::new(),
        revision: String::new(),
        pinned: false,
        folder: path_string(&dir.path().join("unrelated")),
        subdir: String::new(),
    };
    assert!(hub.library_path(&skill).is_err());
    for url in [
        "http://localhost:3000/mcp",
        "http://127.0.0.1/mcp",
        "http://[::1]/mcp",
        "https://example.com/mcp",
    ] {
        assert!(
            crate::access::allow_remote_url(&reqwest::Url::parse(url).unwrap()).is_ok(),
            "{url}"
        );
    }
    for url in [
        "http://example.com/mcp",
        "http://192.168.1.1/mcp",
        "https://user:password@example.com/mcp",
        "file:///tmp/example",
    ] {
        assert!(
            crate::access::allow_remote_url(&reqwest::Url::parse(url).unwrap()).is_err(),
            "{url}"
        );
    }
}

#[test]
fn canonical_skill_fingerprints_are_read_once_per_scan() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let source = dir.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("SKILL.md"), "first").unwrap();
    let alias = dir.path().join("alias");
    std::os::unix::fs::symlink(&source, &alias).unwrap();
    let mut cache = crate::scan::SkillFingerprints::default();
    let first = cache.get(&source).unwrap();
    assert_eq!(cache.get(&alias).unwrap(), first);
    assert_eq!(cache.reads, 1);
    fs::write(source.join("SKILL.md"), "second").unwrap();
    let mut next_scan = crate::scan::SkillFingerprints::default();
    assert_ne!(first, next_scan.get(&source).unwrap());
}

#[test]
fn generated_ipc_contract_is_current() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let path = dir.path().join("bindings.ts");
    crate::ipc::export(&path).unwrap();
    let checked_in = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts");
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        fs::read_to_string(checked_in).unwrap(),
        "Run npm run bindings after changing Rust commands or DTOs"
    );
}

#[test]
fn custom_commands_are_allowed_only_for_local_main_webview() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub_at(dir.path()))
        .invoke_handler(tauri::generate_handler![crate::commands::history])
        .build(crate::context())
        .unwrap();
    let main = app.get_webview_window("main").unwrap_or_else(|| {
        tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap()
    });
    let other = tauri::WebviewWindowBuilder::new(&app, "unprivileged", Default::default())
        .build()
        .unwrap();
    let request = |origin: &str| tauri::webview::InvokeRequest {
        cmd: "history".into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: origin.parse().unwrap(),
        body: tauri::ipc::InvokeBody::default(),
        headers: Default::default(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    };
    assert!(tauri::test::get_ipc_response(&main, request("tauri://localhost")).is_ok());
    assert!(tauri::test::get_ipc_response(&other, request("tauri://localhost")).is_err());
    assert!(tauri::test::get_ipc_response(&main, request("https://example.invalid")).is_err());
}

#[test]
fn capabilities_cover_exactly_the_registered_application_commands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut actual = std::collections::BTreeSet::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let v: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(v["windows"], serde_json::json!(["main"]));
        assert!(v.get("remote").is_none());
        for permission in v["permissions"].as_array().unwrap() {
            actual.insert(permission.as_str().unwrap().to_string());
        }
    }
    let expected: std::collections::BTreeSet<_> = crate::registry::COMMAND_NAMES
        .iter()
        .map(|s| format!("allow-{}", s.replace('_', "-")))
        .collect();
    let core_permissions: std::collections::BTreeSet<_> = actual
        .iter()
        .filter(|permission| permission.starts_with("core:"))
        .cloned()
        .collect();
    assert_eq!(
        core_permissions,
        std::collections::BTreeSet::from(["core:app:allow-version".to_string()])
    );
    actual.retain(|permission| !permission.starts_with("core:"));
    assert_eq!(actual, expected);
}

#[test]
fn export_creation_never_overwrites_and_mcp_environment_is_restricted() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let path = dir.path().join("existing.json");
    fs::write(&path, "user data").unwrap();
    assert!(write_new(&path, b"replacement").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "user data");
    let mut command = tokio::process::Command::new("/usr/bin/env");
    command.env("AGENT_HUB_TEST_SECRET", "never inherit");
    crate::mcp::limited_environment(&mut command);
    assert!(!command
        .as_std()
        .get_envs()
        .any(|(key, _)| key == "AGENT_HUB_TEST_SECRET"));
    assert!(command.as_std().get_envs().all(|(key, _)| [
        "PATH",
        "HOME",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "USER",
        "LOGNAME",
        "SHELL",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "SYSTEMROOT"
    ]
    .iter()
    .any(|allowed| key == *allowed)));
}

#[test]
fn active_work_prevents_exit_until_all_guards_drop() {
    let hub = Hub::new();
    assert!(!hub.busy());
    let first = hub.operation();
    let second = hub.operation();
    assert!(hub.busy());
    drop(first);
    assert!(hub.busy());
    drop(second);
    assert!(!hub.busy());
}

// 资源操作隔离验证：不读写真实 Agent 安装目录。
#[test]
fn audit_remove_skill_preserves_management_copy_and_supports_undo() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let source = hub.home().unwrap().join(".agents/skills/demo");
    fs::create_dir_all(source.join("empty")).unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---\nname: demo\ndescription: audit\n---\nBody",
    )
    .unwrap();
    let content = source_snapshot(&source).unwrap();
    let sid = hash(b"audit-managed-copy");
    let managed = hub.root().unwrap().join("library").join(&sid);
    restore(&managed, &content).unwrap();
    let mut settings = hub.settings().unwrap();
    settings.library.push(Skill {
        id: sid.clone(),
        name: "demo".into(),
        description: "audit".into(),
        source: path_string(&source),
        revision: fingerprint(&content),
        pinned: false,
        folder: path_string(&managed),
        subdir: "".into(),
    });
    settings
        .installations
        .insert(path_string(&source), sid.clone());
    hub.save(&settings).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    let hub = app.state::<Hub>();
    let resource = crate::scan::run(&hub)
        .unwrap()
        .resources
        .into_iter()
        .find(|r| r.path == path_string(&source))
        .unwrap();
    let plan = crate::commands::remove_resource(app.state::<Hub>(), resource.id).unwrap();
    assert!(source.exists(), "preview must not delete");
    hub.apply(&plan.id).unwrap();
    assert!(!source.exists());
    assert!(snapshot(&managed).unwrap() == content);
    assert_eq!(hub.settings().unwrap().library.len(), 1);
    assert!(!hub
        .settings()
        .unwrap()
        .installations
        .contains_key(&path_string(&source)));
    let undo = crate::commands::preview_undo(app.state::<Hub>(), plan.id).unwrap();
    hub.apply(&undo.id).unwrap();
    assert!(snapshot(&source).unwrap() == content);
    assert_eq!(
        hub.settings()
            .unwrap()
            .installations
            .get(&path_string(&source)),
        Some(&sid)
    );
}

#[test]
fn audit_link_entry_removal_is_rejected_without_deleting_source() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let source = hub.home().unwrap().join(".agents/skills/demo");
    let link = hub.home().unwrap().join(".claude/skills/demo");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nBody").unwrap();
    std::os::unix::fs::symlink(&source, &link).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    let hub = app.state::<Hub>();
    let resource = crate::scan::run(&hub)
        .unwrap()
        .resources
        .into_iter()
        .find(|r| r.path == path_string(&link))
        .unwrap();
    let error = crate::commands::remove_resource(app.state::<Hub>(), resource.id)
        .err()
        .unwrap();
    assert!(error.contains("符号链接"));
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(source.join("SKILL.md").exists());
}

#[test]
fn audit_shared_source_removal_must_warn_linked_consumers() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let source = hub.home().unwrap().join(".agents/skills/demo");
    let link = hub.home().unwrap().join(".claude/skills/demo");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nBody").unwrap();
    std::os::unix::fs::symlink(&source, &link).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    let hub = app.state::<Hub>();
    let resource = crate::scan::run(&hub)
        .unwrap()
        .resources
        .into_iter()
        .find(|r| r.path == path_string(&source))
        .unwrap();
    let plan = crate::commands::remove_resource(app.state::<Hub>(), resource.id).unwrap();
    let notes = plan.notes.join("\n");
    assert!(notes.contains("未确认到独立"));
    assert!(notes.contains(&path_string(&link)));
    assert!(
        notes.contains("claude") || notes.contains("Claude"),
        "missing linked Claude consumer in warning: {notes}"
    );
}

#[test]
fn audit_imported_source_remains_associated_after_source_changes() {
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let source = hub.home().unwrap().join(".agents/skills/demo");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nVersion one").unwrap();
    let content = source_snapshot(&source).unwrap();
    let source_string = path_string(&source);
    let sid = hash(format!("{source_string}#").as_bytes());
    let managed = hub.root().unwrap().join("library").join(&sid);
    restore(&managed, &content).unwrap();
    let mut settings = hub.settings().unwrap();
    settings.library.push(Skill {
        id: sid.clone(),
        name: "demo".into(),
        description: "audit".into(),
        source: source_string.clone(),
        revision: fingerprint(&content),
        pinned: false,
        folder: path_string(&managed),
        subdir: "".into(),
    });
    hub.save(&settings).unwrap();
    let resource = crate::scan::run(&hub)
        .unwrap()
        .resources
        .into_iter()
        .find(|r| r.path == source_string)
        .unwrap();
    assert_eq!(resource.library_id, Some(sid.clone()));
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nVersion two").unwrap();
    let resource = crate::scan::run(&hub)
        .unwrap()
        .resources
        .into_iter()
        .find(|r| r.path == source_string)
        .unwrap();
    assert_eq!(
        resource.library_id,
        Some(sid),
        "already-imported local source loses its library association after editing"
    );
}

#[test]
fn audit_remove_mcp_preserves_other_entries_and_undo_for_all_agents() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    let mut settings = hub.settings().unwrap();
    settings.projects.push(path_string(&project));
    hub.save(&settings).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    let hub = app.state::<Hub>();
    for agent in ["claude", "codex", "cursor", "antigravity"] {
        let target = Target {
            agent: agent.into(),
            project: Some(path_string(&project)),
        };
        let path = hub.target(&target, false).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = if agent == "codex" {
            "# root comment\nother = true\n[mcp_servers.demo]\ncommand = 'demo'\n[mcp_servers.keep]\ncommand = 'keep'\ncustom = 42\n"
        } else {
            "{\"other\":true,\"mcpServers\":{\"demo\":{\"command\":\"demo\"},\"keep\":{\"command\":\"keep\",\"custom\":42}}}"
        };
        fs::write(&path, text).unwrap();
        let before = snapshot(&path).unwrap();
        let resource = crate::scan::run(&hub)
            .unwrap()
            .resources
            .into_iter()
            .find(|r| r.kind == "mcp" && r.name == "demo" && r.path == path_string(&path))
            .unwrap();
        let plan = crate::commands::remove_resource(app.state::<Hub>(), resource.id).unwrap();
        assert!(snapshot(&path).unwrap() == before);
        hub.apply(&plan.id).unwrap();
        let value = crate::scan::parse_config(&path, agent).unwrap();
        let servers = crate::scan::servers(&value, agent).unwrap();
        assert!(!servers.contains_key("demo"));
        assert_eq!(servers["keep"]["custom"], 42);
        assert_eq!(value["other"], true);
        let undo = crate::commands::preview_undo(app.state::<Hub>(), plan.id).unwrap();
        hub.apply(&undo.id).unwrap();
        assert!(snapshot(&path).unwrap() == before);
    }
}

#[test]
fn audit_history_undo_and_redo_update_related_records() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let target = hub.home().unwrap().join(".cursor/mcp.json");
    let plan = hub
        .queue(
            "audit history".into(),
            vec![Change {
                path: path_string(&target),
                before: snapshot(&target).unwrap(),
                after: file_snapshot("{\"mcpServers\":{}}".into()),
                sensitive: true,
            }],
            vec![],
        )
        .unwrap();
    hub.apply(&plan.id).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    let undo = crate::commands::preview_undo(app.state::<Hub>(), plan.id.clone()).unwrap();
    let duplicate = crate::commands::preview_undo(app.state::<Hub>(), plan.id.clone()).unwrap();
    app.state::<Hub>().apply(&undo.id).unwrap();
    assert!(app
        .state::<Hub>()
        .apply(&duplicate.id)
        .err()
        .unwrap()
        .contains("已经撤销"));
    assert!(!target.exists());
    let error = crate::commands::preview_undo(app.state::<Hub>(), plan.id.clone())
        .err()
        .unwrap();
    assert!(error.contains("已经撤销"));
    let history = app.state::<Hub>().records().unwrap();
    assert_eq!(
        history.iter().find(|h| h.id == plan.id).unwrap().undone_by,
        Some(undo.id.clone())
    );
    let redo = crate::commands::preview_undo(app.state::<Hub>(), undo.id).unwrap();
    app.state::<Hub>().apply(&redo.id).unwrap();
    assert!(target.exists());
    assert!(app
        .state::<Hub>()
        .records()
        .unwrap()
        .iter()
        .find(|h| h.id == plan.id)
        .unwrap()
        .undone_by
        .is_none());
}

#[test]
fn audit_history_removed_project_blocks_undo_without_touching_files() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    let mut settings = hub.settings().unwrap();
    settings.projects.push(path_string(&project));
    hub.save(&settings).unwrap();
    let target = project.join(".cursor/mcp.json");
    let plan = hub
        .queue(
            "project change".into(),
            vec![Change {
                path: path_string(&target),
                before: snapshot(&target).unwrap(),
                after: file_snapshot("{\"mcpServers\":{}}".into()),
                sensitive: true,
            }],
            vec![],
        )
        .unwrap();
    hub.apply(&plan.id).unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    crate::commands::remove_project(app.state::<Hub>(), path_string(&project)).unwrap();
    let content = snapshot(&target).unwrap();
    let entries = crate::commands::history(app.state::<Hub>()).unwrap();
    assert_eq!(entries[0].required_projects, vec![path_string(&project)]);
    let error = crate::commands::preview_undo(app.state::<Hub>(), plan.id.clone())
        .err()
        .unwrap();
    assert!(error.contains("超出"));
    assert!(snapshot(&target).unwrap() == content);
    // Re-adding the same authorized project restores undo access.
    let mut settings = app.state::<Hub>().settings().unwrap();
    settings.projects.push(path_string(&project));
    app.state::<Hub>().save(&settings).unwrap();
    assert!(crate::commands::preview_undo(app.state::<Hub>(), plan.id).is_ok());
}

#[test]
fn audit_backup_roundtrip_preserves_skill_but_not_machine_state() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(&dir.path().join("original"));
    let source = hub.home().unwrap().join(".agents/skills/demo");
    fs::create_dir_all(source.join("empty")).unwrap();
    fs::write(
        source.join("SKILL.md"),
        "---\nname: demo\n---\nBackup audit",
    )
    .unwrap();
    let content = source_snapshot(&source).unwrap();
    let sid = hash(format!("{}#", path_string(&source)).as_bytes());
    let managed = hub.root().unwrap().join("library").join(&sid);
    restore(&managed, &content).unwrap();
    let project = dir.path().join("project");
    fs::create_dir_all(&project).unwrap();
    let mut settings = hub.settings().unwrap();
    settings.projects.push(path_string(&project));
    settings.library.push(Skill {
        id: sid.clone(),
        name: "demo".into(),
        description: "audit".into(),
        source: path_string(&source),
        revision: fingerprint(&content),
        pinned: true,
        folder: path_string(&managed),
        subdir: "".into(),
    });
    hub.save(&settings).unwrap();
    fs::write(hub.home().unwrap().join(".claude.json"), "{\"mcpServers\":{\"secret\":{\"command\":\"demo\",\"env\":{\"TOKEN\":\"not-in-export-audit-secret\"}}}}").unwrap();
    let backup = dir.path().join("backup.json");
    hub.grant(crate::access::PathPurpose::Export, backup.clone())
        .unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    crate::commands::export_library(app.state::<Hub>(), path_string(&backup)).unwrap();
    let bytes = fs::read(&backup).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("not-in-export-audit-secret"));
    assert!(crate::commands::export_library(app.state::<Hub>(), path_string(&backup)).is_err());
    assert_eq!(fs::read(&backup).unwrap(), bytes);
    let restored = hub_at(&dir.path().join("restored"));
    restored
        .grant(crate::access::PathPurpose::Import, backup.clone())
        .unwrap();
    let app2 = tauri::test::mock_builder()
        .manage(restored)
        .build(crate::context())
        .unwrap();
    let plan =
        crate::commands::import_library(app2.state::<Hub>(), path_string(&backup), false).unwrap();
    assert!(app2.state::<Hub>().settings().unwrap().library.is_empty());
    app2.state::<Hub>().apply(&plan.id).unwrap();
    let settings2 = app2.state::<Hub>().settings().unwrap();
    assert_eq!(settings2.library.len(), 1);
    let skill = &settings2.library[0];
    assert!(skill.pinned);
    assert!(snapshot(Path::new(&skill.folder)).unwrap() == content);
    assert!(settings2.projects.is_empty());
    assert!(settings2.installations.is_empty());
    assert!(!app2
        .state::<Hub>()
        .home()
        .unwrap()
        .join(".claude.json")
        .exists());
    fs::write(Path::new(&skill.folder).join("SKILL.md"), "local edit").unwrap();
    let skipped = crate::commands::import_library(app2.state::<Hub>(), path_string(&backup), false)
        .err()
        .unwrap();
    assert!(skipped.contains("已跳过 1"));
    assert_eq!(
        fs::read_to_string(Path::new(&skill.folder).join("SKILL.md")).unwrap(),
        "local edit"
    );
    let overwrite =
        crate::commands::import_library(app2.state::<Hub>(), path_string(&backup), true).unwrap();
    assert!(overwrite.notes.join("\n").contains("覆盖 1 项"));
    app2.state::<Hub>().apply(&overwrite.id).unwrap();
    assert!(snapshot(Path::new(&skill.folder)).unwrap() == content);
    let undo = crate::commands::preview_undo(app2.state::<Hub>(), plan.id).unwrap();
    app2.state::<Hub>().apply(&undo.id).unwrap();
    assert!(app2.state::<Hub>().settings().unwrap().library.is_empty());
}

#[test]
fn history_recovery_warns_about_new_edits_and_rechecks_before_apply() {
    use tauri::Manager;
    let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let hub = hub_at(dir.path());
    let path = hub.home().unwrap().join(".claude.json");
    fs::write(&path, "before").unwrap();
    let plan = hub
        .queue(
            "failure recovery fixture".into(),
            vec![Change {
                path: path_string(&path),
                before: snapshot(&path).unwrap(),
                after: file_snapshot("after".into()),
                sensitive: true,
            }],
            vec![],
        )
        .unwrap();
    hub.apply(&plan.id).unwrap();
    let mut records = hub.records().unwrap();
    records[0].status = "failed".into();
    hub.save_records(&records).unwrap();
    fs::write(&path, "later edit").unwrap();
    let app = tauri::test::mock_builder()
        .manage(hub)
        .build(crate::context())
        .unwrap();
    let recovery = crate::commands::preview_undo(app.state::<Hub>(), plan.id).unwrap();
    assert!(recovery
        .notes
        .iter()
        .any(|s| s.contains("存在后续修改") && s.contains(&path_string(&path))));
    fs::write(&path, "even later").unwrap();
    assert!(app
        .state::<Hub>()
        .apply(&recovery.id)
        .err()
        .unwrap()
        .contains("外部修改"));
    assert_eq!(fs::read_to_string(path).unwrap(), "even later");
}

#[test]
fn history_old_records_remain_readable() {
    let record: History = serde_json::from_str(
        r#"{"id":"old","title":"old change","time":"0","status":"applied","paths":[]}"#,
    )
    .unwrap();
    assert!(record.undo_of.is_none() && record.undone_by.is_none());
}
