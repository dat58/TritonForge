//! Model group domain types and mythology name generator.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Opaque group identifier wrapping a UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub Uuid);

impl GroupId {
    /// Generates a new random group identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for GroupId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for GroupId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

/// Origin job reference stored per group member so source files can be deleted on group delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelGroupMember {
    /// ID of the conversion job that produced this model.
    pub job_id: String,
    /// Triton model name (directory name under the job output).
    pub model_name: String,
}

/// A named collection of TRT models copied into a shared deployment directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelGroup {
    /// Unique identifier for this group.
    pub id: GroupId,
    /// Human-readable group name (unique across all groups).
    pub name: String,
    /// Absolute path to the groups directory for this group.
    pub dir_path: PathBuf,
    /// Models that have been copied into this group.
    pub members: Vec<ModelGroupMember>,
    /// When the group was created.
    pub created_at: DateTime<Utc>,
    /// When the group was last modified.
    pub updated_at: DateTime<Utc>,
}

const MYTHOLOGY_NAMES: &[&str] = &[
    // Egyptian
    "Osiris",
    "Anubis",
    "Horus",
    "Ra",
    "Thoth",
    "Bastet",
    "Isis",
    "Seth",
    "Nephthys",
    "Sobek",
    "Khnum",
    "Ptah",
    "Sekhmet",
    "Hathor",
    "Nut",
    // Norse
    "Odin",
    "Thor",
    "Freya",
    "Loki",
    "Baldr",
    "Tyr",
    "Frigg",
    "Heimdall",
    "Njord",
    "Frey",
    "Skadi",
    "Sigyn",
    "Bragi",
    "Idun",
    "Vidar",
    // Greek
    "Zeus",
    "Athena",
    "Apollo",
    "Artemis",
    "Hermes",
    "Ares",
    "Hephaestus",
    "Aphrodite",
    "Poseidon",
    "Demeter",
    "Dionysus",
    "Hades",
    "Persephone",
    "Hestia",
    "Heracles",
];

/// Returns a random mythology name using subsecond nanos as a seed (no extra dependencies).
pub fn random_mythology_name() -> String {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(7);
    MYTHOLOGY_NAMES[seed % MYTHOLOGY_NAMES.len()].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_mythology_name_returns_known_name() {
        let name = random_mythology_name();
        assert!(MYTHOLOGY_NAMES.contains(&name.as_str()));
    }

    #[test]
    fn group_id_roundtrips_through_string() {
        let id = GroupId::new();
        let s = id.to_string();
        let parsed: GroupId = s.parse().expect("parse group id");
        assert_eq!(id, parsed);
    }
}
