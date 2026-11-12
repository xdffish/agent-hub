use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Serialize, Deserialize, specta::Type)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub revision: String,
    pub pinned: bool,
    pub folder: String,
    #[serde(default)]
    pub subdir: String,
}

#[derive(Clone, Serialize, specta::Type)]
pub struct Resource {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub canonical: String,
    pub is_link: bool,
    pub scope: String,
    pub agents: Vec<String>,
    pub fingerprint: String,
    pub changed: bool,
    pub library_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Serialize, specta::Type)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub detected: bool,
    pub evidence: String,
    pub guide: String,
}

#[derive(Clone, Serialize, specta::Type)]
pub struct Scan {
    pub agents: Vec<Agent>,
    pub resources: Vec<Resource>,
    pub library: Vec<Skill>,
    pub projects: Vec<String>,
    pub warnings: Vec<String>,
    pub scanned_at: String,
    pub data_dir: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub projects: Vec<String>,
    pub library: Vec<Skill>,
    #[serde(default)]
    pub installations: BTreeMap<String, String>,
    #[serde(skip)]
    pub observations: BTreeMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub exists: bool,
    pub directory: bool,
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub directories: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub before: Snapshot,
    pub after: Snapshot,
    pub sensitive: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default)]
    pub undo_of: Option<String>,
    pub id: String,
    pub title: String,
    pub changes: Vec<Change>,
    pub notes: Vec<String>,
    pub created_at: String,
}

#[derive(Clone, Serialize, specta::Type)]
pub struct ChangeView {
    pub path: String,
    pub diff: String,
}

#[derive(Clone, Serialize, specta::Type)]
pub struct PlanView {
    pub id: String,
    pub title: String,
    pub changes: Vec<ChangeView>,
    pub notes: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, specta::Type)]
pub struct History {
    #[serde(default)]
    pub required_projects: Vec<String>,
    #[serde(default)]
    pub undo_of: Option<String>,
    #[serde(default)]
    pub undone_by: Option<String>,
    pub id: String,
    pub title: String,
    pub time: String,
    pub status: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, specta::Type)]
pub struct Target {
    pub agent: String,
    pub project: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BundleSkill {
    pub skill: Skill,
    pub content: Snapshot,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub format: u32,
    pub skills: Vec<BundleSkill>,
    #[serde(default)]
    pub assignments: Vec<BundleAssignment>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BundleAssignment {
    pub skill_id: String,
    pub target: Target,
}
