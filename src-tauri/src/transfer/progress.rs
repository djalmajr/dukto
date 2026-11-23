use std::{collections::HashSet, time::Instant};

use crate::protocol::types::{
    ItemKind, ItemMetadata, TransferHeader, TransferItemDescriptor, TransferItemProgress,
    TransferProgress,
};

/// Maps sequential wire entries back to the roots selected by the user.
pub(super) struct ProgressTracker {
    transfer_id: String,
    total_size: u64,
    items: Vec<TransferItemDescriptor>,
    legacy: bool,
    index: usize,
    entries_completed: u32,
    item_start: u64,
    started: Instant,
}

impl ProgressTracker {
    pub fn new(header: &TransferHeader) -> Result<Self, String> {
        if !header.selected_items.is_empty() {
            let mut ids = HashSet::new();
            let mut count = 0u32;
            let mut size = 0u64;
            for item in &header.selected_items {
                if item.item_id.is_empty()
                    || !ids.insert(&item.item_id)
                    || item.item_count == 0
                    || (item.kind == ItemKind::File && item.item_count != 1)
                {
                    return Err("Invalid selected item manifest".into());
                }
                count = count
                    .checked_add(item.item_count)
                    .ok_or("Item count overflow")?;
                size = size
                    .checked_add(item.size_bytes)
                    .ok_or("Item size overflow")?;
            }
            if count != header.item_count || size != header.total_size {
                return Err("Selected items do not match transfer totals".into());
            }
        }
        let mut items = header.selected_items.clone();
        for item in &mut items {
            item.name = display_name(&item.name);
        }
        Ok(Self {
            transfer_id: header.transfer_id.clone(),
            total_size: header.total_size,
            legacy: items.is_empty(),
            items,
            index: 0,
            entries_completed: 0,
            item_start: 0,
            started: Instant::now(),
        })
    }

    pub fn initial(&mut self) -> TransferProgress {
        self.started = Instant::now();
        let mut progress = self.snapshot(0, false);
        progress.item_progress = None;
        progress.item_name = None;
        if !self.items.is_empty() {
            progress.items = Some(self.items.clone());
        }
        progress
    }

    pub fn begin_entry(
        &mut self,
        metadata: &ItemMetadata,
        bytes: u64,
    ) -> Result<TransferProgress, String> {
        if self.legacy {
            self.items.push(TransferItemDescriptor {
                item_id: format!("legacy-{}", self.index),
                name: display_name(&metadata.name),
                kind: metadata.kind.clone(),
                size_bytes: metadata.size_bytes,
                item_count: 1,
            });
        }
        let item = self
            .items
            .get(self.index)
            .ok_or("No selected item for wire entry")?;
        let remaining = item
            .size_bytes
            .saturating_sub(bytes.saturating_sub(self.item_start));
        if metadata.size_bytes > remaining
            || (item.kind == ItemKind::File && metadata.kind != ItemKind::File)
        {
            return Err("Entry does not match selected item".into());
        }
        let mut progress = self.snapshot(bytes, false);
        progress.item_name = Some(item.name.clone());
        if self.legacy {
            progress.items = Some(vec![item.clone()]);
        }
        Ok(progress)
    }

    pub fn snapshot(&self, bytes: u64, completed: bool) -> TransferProgress {
        let elapsed = self.started.elapsed().as_secs_f64();
        let item = self.items.get(self.index);
        TransferProgress {
            transfer_id: self.transfer_id.clone(),
            bytes_sent: bytes,
            bytes_total: self.total_size,
            speed_bps: if elapsed > 0.0 {
                (bytes as f64 / elapsed) as u64
            } else {
                0
            },
            percent: if self.total_size == 0 {
                100.0
            } else {
                bytes as f64 / self.total_size as f64 * 100.0
            },
            item_name: None,
            items: None,
            item_progress: item.map(|item| TransferItemProgress {
                item_id: item.item_id.clone(),
                bytes_sent: bytes.saturating_sub(self.item_start),
                bytes_total: item.size_bytes,
                completed,
            }),
        }
    }

    pub fn finish_entry(&mut self, bytes: u64) -> Result<TransferProgress, String> {
        let item = self
            .items
            .get(self.index)
            .ok_or("No selected item to finish")?;
        self.entries_completed += 1;
        let completed = self.entries_completed == item.item_count;
        if completed && bytes.saturating_sub(self.item_start) != item.size_bytes {
            return Err("Selected item byte count does not match manifest".into());
        }
        let progress = self.snapshot(bytes, completed);
        if completed {
            self.index += 1;
            self.entries_completed = 0;
            self.item_start = bytes;
        }
        Ok(progress)
    }
}

fn display_name(name: &str) -> String {
    name.rsplit(['/', '\\']).next().unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> TransferHeader {
        TransferHeader {
            transfer_id: "test".into(),
            sender_device_id: "peer".into(),
            sender: None,
            item_count: 2,
            total_size: 8,
            selected_items: vec![TransferItemDescriptor {
                item_id: "folder".into(),
                name: "folder".into(),
                kind: ItemKind::Directory,
                size_bytes: 8,
                item_count: 2,
            }],
        }
    }

    fn metadata(size_bytes: u64) -> ItemMetadata {
        ItemMetadata {
            item_id: "same-wire-id".into(),
            name: "nested/file.txt".into(),
            kind: ItemKind::File,
            relative_path: Some("nested/file.txt".into()),
            size_bytes,
            modified_at: None,
        }
    }

    #[test]
    fn manifest_cannot_disagree_with_transfer_or_entry_totals() {
        let mut invalid = header();
        invalid.selected_items[0].size_bytes = 9;
        assert!(ProgressTracker::new(&invalid).is_err());
        invalid = header();
        invalid.selected_items[0].item_count = 1;
        assert!(ProgressTracker::new(&invalid).is_err());
        invalid = header();
        invalid
            .selected_items
            .push(invalid.selected_items[0].clone());
        invalid.item_count = 4;
        invalid.total_size = 16;
        assert!(ProgressTracker::new(&invalid).is_err());

        let mut tracker = ProgressTracker::new(&header()).unwrap();
        assert!(tracker.begin_entry(&metadata(9), 0).is_err());
        tracker.begin_entry(&metadata(3), 0).unwrap();
        assert!(
            !tracker
                .finish_entry(3)
                .unwrap()
                .item_progress
                .unwrap()
                .completed
        );
        tracker.begin_entry(&metadata(4), 3).unwrap();
        assert!(tracker.finish_entry(7).is_err());
    }

    #[test]
    fn legacy_entries_are_announced_independently_and_empty_files_finish_explicitly() {
        let mut header = header();
        header.selected_items.clear();
        let mut tracker = ProgressTracker::new(&header).unwrap();
        assert!(tracker.initial().items.is_none());
        let first = tracker.begin_entry(&metadata(8), 0).unwrap();
        let first_id = first.items.as_ref().unwrap()[0].item_id.clone();
        assert_eq!(first.items.unwrap()[0].name, "file.txt");
        assert_eq!(
            tracker.snapshot(4, false).item_progress.unwrap().bytes_sent,
            4
        );
        assert!(
            tracker
                .finish_entry(8)
                .unwrap()
                .item_progress
                .unwrap()
                .completed
        );
        let empty = tracker.begin_entry(&metadata(0), 8).unwrap();
        assert_ne!(empty.items.unwrap()[0].item_id, first_id);
        assert!(!empty.item_progress.unwrap().completed);
        let completed = tracker.finish_entry(8).unwrap().item_progress.unwrap();
        assert!(completed.completed);
        assert_eq!((completed.bytes_sent, completed.bytes_total), (0, 0));
    }
}
