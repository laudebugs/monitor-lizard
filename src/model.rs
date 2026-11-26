use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Scope { Loopback, Lan, Public }

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Direction { Listen, Outbound, Local }

#[derive(Clone, Serialize, Debug)]
pub struct Proc {
    pub pid: u32, pub ppid: u32, pub user: String, pub cpu: f32, pub rss_kb: u64,
    pub command: String, pub exe: String,
}

#[derive(Clone, Serialize, Debug)]
pub struct Conn {
    pub pid: u32, pub name: String, pub proto: String, pub state: String,
    pub local_addr: String, pub local_port: u16,
    pub remote_addr: Option<String>, pub remote_port: Option<u16>,
    pub direction: Direction, pub scope: Scope,
    /// listening on all interfaces, i.e. reachable from the LAN
    pub exposed: bool,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct Signing { pub signed: bool, pub team: Option<String>, pub authority: Option<String>, pub apple: bool }

/// How far an app group can be trusted, decided by rules only (see `rules::classify`).
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Label { Trusted, Unknown, Suspicious }

#[derive(Serialize, Debug)]
pub struct Group {
    pub key: String, pub label: String, pub category: String,
    pub procs: Vec<Proc>, pub conns: Vec<Conn>,
    pub signing: Option<Signing>, pub approved: bool,
    /// `trust` and `rules_fired` are filled in by `snapshot::take` after the rules run.
    pub trust: Label, pub rules_fired: Vec<&'static str>,
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity { High, Medium, Low }

#[derive(Serialize, Debug)]
pub struct Finding { pub severity: Severity, pub group: String, pub group_key: String, pub rule: &'static str, pub detail: String }

impl Finding {
    /// Stable across polls: the same group and rule is the same finding even when its detail text changes.
    pub fn id(&self) -> String { format!("{}:{}", self.group_key, self.rule) }
}

#[derive(Serialize, Debug)]
pub struct Totals { pub processes: usize, pub connections: usize, pub groups: usize, pub listening: usize, pub outbound: usize, pub findings: usize }

#[derive(Serialize, Debug)]
pub struct Snapshot {
    pub at: u64, pub groups: Vec<Group>, pub findings: Vec<Finding>, pub totals: Totals,
    /// Findings the user marked as expected: still shown, but not counted, notified or used for labels.
    pub expected_findings: Vec<Finding>,
}
