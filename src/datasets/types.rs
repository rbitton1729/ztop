//! Plain-Rust domain types representing a snapshot of dataset state. No
//! libzfs types leak through this interface — callers upstream
//! (`app.rs`, `ui/*`, tests) see only the shapes defined here. This
//! decoupling is load-bearing for (a) unit-testing the app and UI against
//! `FakeDatasetsSource`, and (b) the future v1.0 fleet/remote mode where
//! dataset data may arrive as serialized `DatasetNode` over SSH rather
//! than from a local libzfs call.

use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct DatasetNode {
    /// Full ZFS name: "tank/home/alice". The tree's structural
    /// information is preserved in `children`; the full name is kept on
    /// every node so the UI can render it without reconstructing from the
    /// parent chain and so `Detail::name` (in `DatasetsView`) maps
    /// unambiguously across refreshes.
    pub name: String,
    pub kind: DatasetKind,
    pub used_bytes: u64,
    pub refer_bytes: u64,
    pub available_bytes: u64,
    /// 1.0 == no savings; 1.42 == 42% savings; 0.0 if libzfs reports
    /// the unavailable sentinel.
    pub compression_ratio: f64,
    pub properties: DatasetProperties,
    /// Alphabetically sorted by `name`. Empty for zvols and for
    /// filesystems with no nested children.
    pub children: Vec<DatasetNode>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DatasetKind {
    Filesystem,
    Volume,
}

/// Native ZFS properties shown in the detail drilldown. Optional fields
/// are `None` when libzfs returns the inherited / default / "none"
/// sentinel.
#[derive(Clone, Debug, Default)]
pub struct DatasetProperties {
    pub mountpoint: Option<String>,
    pub compression_algorithm: Option<String>,
    pub recordsize_bytes: Option<u64>,
    pub volblocksize_bytes: Option<u64>,
    pub atime_on: Option<bool>,
    pub sync_mode: Option<String>,
    pub snapdir_visible: Option<bool>,
    pub quota_bytes: Option<u64>,
    pub refquota_bytes: Option<u64>,
    pub reservation_bytes: Option<u64>,
    pub refreservation_bytes: Option<u64>,
    pub dedup_on: Option<bool>,
    pub copies: Option<u8>,
    pub encryption_algorithm: Option<String>,
    pub creation_time: Option<SystemTime>,
}

impl DatasetNode {
    /// True if this node has children to expand into. Always false for
    /// volumes; reflects `children.is_empty()` for filesystems.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs(name: &str, children: Vec<DatasetNode>) -> DatasetNode {
        DatasetNode {
            name: name.into(),
            kind: DatasetKind::Filesystem,
            used_bytes: 0,
            refer_bytes: 0,
            available_bytes: 0,
            compression_ratio: 1.0,
            properties: DatasetProperties::default(),
            children,
        }
    }

    fn zvol(name: &str) -> DatasetNode {
        DatasetNode {
            name: name.into(),
            kind: DatasetKind::Volume,
            used_bytes: 0,
            refer_bytes: 0,
            available_bytes: 0,
            compression_ratio: 1.0,
            properties: DatasetProperties::default(),
            children: vec![],
        }
    }

    #[test]
    fn has_children_true_for_filesystem_with_kids() {
        let n = fs("tank", vec![fs("tank/home", vec![])]);
        assert!(n.has_children());
    }

    #[test]
    fn has_children_false_for_empty_filesystem() {
        let n = fs("tank/empty", vec![]);
        assert!(!n.has_children());
    }

    #[test]
    fn has_children_false_for_zvol() {
        let n = zvol("tank/swap");
        assert!(!n.has_children());
    }
}
