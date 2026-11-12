use crate::{files::*, model::*};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

pub struct Hub {
    pub lock: Mutex<()>,
    pub plans: Mutex<BTreeMap<String, Plan>>,
    pub storage: Option<PathBuf>,
    pub grants: Mutex<Vec<crate::access::Grant>>,
    active_operations: AtomicUsize,
    #[cfg(test)]
    pub home_override: Option<PathBuf>,
}
impl Hub {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
            plans: Mutex::new(BTreeMap::new()),
            storage: None,
            grants: Mutex::new(vec![]),
            active_operations: AtomicUsize::new(0),
            #[cfg(test)]
            home_override: None,
        }
    }
    pub fn operation(&self) -> Operation<'_> {
        self.active_operations.fetch_add(1, Ordering::AcqRel);
        Operation(&self.active_operations)
    }
    pub fn busy(&self) -> bool {
        self.active_operations.load(Ordering::Acquire) > 0 || self.lock.try_lock().is_err()
    }
    pub fn home(&self) -> Result<PathBuf> {
        #[cfg(test)]
        if let Some(p) = &self.home_override {
            return Ok(p.clone());
        }
        dirs::home_dir().ok_or("无法定位用户目录".into())
    }
    pub fn root(&self) -> Result<PathBuf> {
        let root = match &self.storage {
            Some(p) => p.clone(),
            None => dirs::data_local_dir()
                .ok_or("无法定位应用目录")?
                .join("dev.agenthub.desktop"),
        };
        ensure_no_symlink_ancestors(&root)?;
        private_dir(&root)?;
        Ok(root)
    }
    pub fn settings_path(&self) -> Result<PathBuf> {
        Ok(self.root()?.join("settings.json"))
    }
    pub fn settings(&self) -> Result<Settings> {
        let p = self.settings_path()?;
        let mut settings: Settings = if p.exists() {
            serde_json::from_slice(&fs::read(p).map_err(|_| "读取管理库失败")?)
                .map_err(|_| "管理库格式错误，请恢复备份，不会覆盖现有文件")?
        } else {
            Settings::default()
        };
        let observations = self.root()?.join("observations.json");
        if observations.exists() {
            settings.observations =
                serde_json::from_slice(&fs::read(observations).map_err(|_| "读取盘点基准失败")?)
                    .map_err(|_| "盘点基准格式错误")?;
        }
        Ok(settings)
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        atomic_write(
            &self.settings_path()?,
            &serde_json::to_vec_pretty(settings).map_err(|_| "序列化失败")?,
            false,
        )
    }
    pub fn settings_change(&self, settings: &Settings) -> Result<Change> {
        let p = self.settings_path()?;
        Ok(Change {
            path: path_string(&p),
            before: snapshot(&p)?,
            after: file_snapshot(serde_json::to_string_pretty(settings).map_err(|_| "序列化失败")?),
            sensitive: false,
        })
    }
    pub fn queue(
        &self,
        title: String,
        changes: Vec<Change>,
        notes: Vec<String>,
    ) -> Result<PlanView> {
        let changes: Vec<_> = changes
            .into_iter()
            .filter(|c| c.before != c.after)
            .collect();
        if changes.is_empty() {
            return Err("目标与所选内容一致，无需变更".into());
        }
        let plan = Plan {
            undo_of: None,
            id: id(),
            title,
            changes,
            notes,
            created_at: now(),
        };
        let view = plan_view(&plan);
        let mut plans = self.plans.lock().map_err(|_| "预览锁不可用")?;
        if plans.len() > 20 {
            plans.clear();
        }
        plans.insert(plan.id.clone(), plan);
        Ok(view)
    }
    pub fn records(&self) -> Result<Vec<History>> {
        let p = self.root()?.join("history.json");
        if !p.exists() {
            return Ok(vec![]);
        }
        serde_json::from_slice(&fs::read(p).map_err(|_| "读取历史失败")?)
            .map_err(|_| "历史记录格式错误".into())
    }
    pub fn save_records(&self, records: &[History]) -> Result<()> {
        atomic_write(
            &self.root()?.join("history.json"),
            &serde_json::to_vec_pretty(records).map_err(|_| "保存历史失败")?,
            false,
        )
    }
    pub fn target(&self, target: &Target, skills: bool) -> Result<PathBuf> {
        let home = self.home()?;
        let settings = self.settings()?;
        let project = match &target.project {
            Some(p) => {
                if !settings.projects.contains(p) {
                    return Err("项目尚未添加".into());
                }
                Some(checked_path(p)?)
            }
            None => None,
        };
        let base = project.as_ref().unwrap_or(&home);
        let relative = match (target.agent.as_str(), project.is_some(), skills) {
            ("claude", _, true) => ".claude/skills",
            ("codex" | "cursor", _, true) => ".agents/skills",
            ("antigravity", true, true) => ".agents/skills",
            ("antigravity", false, true) => ".gemini/config/skills",
            ("claude", false, false) => ".claude.json",
            ("claude", true, false) => ".mcp.json",
            ("codex", _, false) => ".codex/config.toml",
            ("cursor", _, false) => ".cursor/mcp.json",
            ("antigravity", false, false) => ".gemini/config/mcp_config.json",
            ("antigravity", true, false) => ".agents/mcp_config.json",
            _ => return Err("未知 Agent".into()),
        };
        if target.agent == "codex" && project.is_none() && !skills {
            if let Some(custom) = std::env::var_os("CODEX_HOME") {
                return Ok(checked_path(&custom.to_string_lossy())?.join("config.toml"));
            }
        }
        Ok(base.join(relative))
    }
    pub fn apply(&self, plan_id: &str) -> Result<History> {
        let plan = self
            .plans
            .lock()
            .map_err(|_| "预览锁不可用")?
            .remove(plan_id)
            .ok_or("预览已失效，请重新预览")?;
        let mut records = self.records()?;
        if let Some(parent) = &plan.undo_of {
            let original = records
                .iter()
                .find(|r| &r.id == parent)
                .ok_or("原操作记录不存在")?;
            if original.undone_by.is_some() {
                return Err("此操作已经撤销或恢复，请刷新历史".into());
            }
        }
        for c in &plan.changes {
            self.authorize_change(c)?;
            let p = checked_path(&c.path)?;
            ensure_no_symlink_ancestors(&p)?;
            if snapshot(&p)? != c.before {
                return Err(format!("发现外部修改，请重新预览：{}", c.path));
            }
        }
        let backup_dir = self.root()?.join("backups");
        private_dir(&backup_dir)?;
        atomic_write(
            &backup_dir.join(format!("{}.json", plan.id)),
            &serde_json::to_vec(&plan).map_err(|_| "备份失败")?,
            false,
        )?;
        let mut record = History {
            required_projects: vec![],
            undo_of: plan.undo_of.clone(),
            undone_by: None,
            id: plan.id.clone(),
            title: plan.title.clone(),
            time: now(),
            status: "applying".into(),
            paths: plan.changes.iter().map(|c| c.path.clone()).collect(),
        };
        records.insert(0, record.clone());
        self.save_records(&records)?;
        for (i, c) in plan.changes.iter().enumerate() {
            let p = Path::new(&c.path);
            // Recheck each target immediately before mutation, in addition to preflight.
            let result = ensure_no_symlink_ancestors(p).and_then(|_| {
                self.authorize_change(c)?;
                if snapshot(p)? != c.before {
                    return Err("目标在应用期间被外部修改".into());
                }
                restore(p, &c.after)
            });
            if let Err(error) = result {
                let mut failures = vec![];
                for previous in plan.changes[..i].iter().rev() {
                    let q = Path::new(&previous.path);
                    if snapshot(q).ok().as_ref() == Some(&previous.after) {
                        if restore(q, &previous.before).is_err() {
                            failures.push(previous.path.clone());
                        }
                    } else {
                        failures.push(previous.path.clone());
                    }
                }
                // The failed target may be partially written. Preserve it and its full preimage backup.
                record.status = "failed".into();
                records[0] = record;
                self.save_records(&records)?;
                return Err(format!("应用失败：{error}。备份编号 {}；失败目标可能需手工恢复，另有 {} 个目标未能自动回退。", plan.id, failures.len()));
            }
        }
        record.status = "applied".into();
        records[0] = record.clone();
        // Reversing an undo reactivates its parent; follow the chain for repeated undo/redo.
        let mut ancestor = plan.undo_of.clone();
        let mut inactive = true;
        let mut seen = std::collections::BTreeSet::new();
        while let Some(id) = ancestor {
            if !seen.insert(id.clone()) {
                break;
            }
            let Some(entry) = records.iter_mut().find(|r| r.id == id) else {
                break;
            };
            entry.undone_by = if inactive {
                Some(plan.id.clone())
            } else {
                None
            };
            ancestor = entry.undo_of.clone();
            inactive = !inactive;
        }
        self.save_records(&records)?;
        Ok(record)
    }
}

pub struct Operation<'a>(&'a AtomicUsize);
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn snapshot_display(s: &Snapshot, sensitive: bool) -> String {
    if !s.exists {
        return String::new();
    }
    if s.directory {
        return s
            .entries
            .iter()
            .map(|e| {
                let mut summary = format!(
                    "{}  {} bytes  sha256:{}{}\n",
                    e.path,
                    e.bytes.len(),
                    &hash(&e.bytes)[..12],
                    if e.executable { " executable" } else { "" }
                );
                if e.path == "SKILL.md" {
                    if let Ok(text) = std::str::from_utf8(&e.bytes) {
                        summary.push_str(&text.chars().take(24000).collect::<String>());
                        summary.push('\n');
                    }
                }
                summary
            })
            .collect();
    }
    let text = text_of(s).unwrap_or_else(|_| "[二进制文件]".into());
    if !sensitive {
        return text;
    }
    let value = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        v
    } else if let Ok(v) = toml_edit::de::from_str::<serde_json::Value>(&text) {
        v
    } else {
        return "[无法解析的配置，内容隐藏]".into();
    };
    fn hide(v: &serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Object(m) => {
                serde_json::Value::Object(m.iter().map(|(k, v)| (k.clone(), hide(v))).collect())
            }
            serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(hide).collect()),
            serde_json::Value::String(_) => serde_json::json!("[值已隐藏]"),
            _ => v.clone(),
        }
    }
    serde_json::to_string_pretty(&hide(&value)).unwrap_or_default()
}
pub fn plan_view(plan: &Plan) -> PlanView {
    PlanView {
        id: plan.id.clone(),
        title: plan.title.clone(),
        notes: plan.notes.clone(),
        changes: plan
            .changes
            .iter()
            .map(|c| {
                let before = snapshot_display(&c.before, c.sensitive);
                let after = snapshot_display(&c.after, c.sensitive);
                let mut diff = similar::TextDiff::from_lines(&before, &after)
                    .unified_diff()
                    .header("当前", "应用后")
                    .to_string();
                if diff.is_empty() && c.before != c.after {
                    diff = "字段值发生变化；为保护凭据，所有字符串值均隐藏。\n".into();
                }
                ChangeView {
                    path: c.path.clone(),
                    diff,
                }
            })
            .collect(),
    }
}
