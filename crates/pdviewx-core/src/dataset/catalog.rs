//! Immutable hierarchy metadata with compact, allocation-free traversal.
//!
//! Construction costs `O(chunks log chunks)` and stores parent-child edges in
//! compressed rows. Lookup costs `O(log chunks)`; iterating roots or children
//! allocates nothing and costs `O(returned chunks)`.

use crate::{ChunkDescriptor, ChunkFootprint, ChunkId, DatasetError, DatasetId};

#[derive(Clone, Copy, Debug, Default)]
struct ChildRange {
    start: u32,
    count: u32,
}

/// Validated metadata for one logical, potentially out-of-core dataset.
#[derive(Clone, Debug)]
pub struct DatasetCatalog {
    dataset: DatasetId,
    descriptors: Vec<ChunkDescriptor>,
    roots: Vec<u32>,
    child_ranges: Vec<ChildRange>,
    children: Vec<u32>,
    provider: Option<pdbiox::DatasetDescriptor>,
    logical_chunk_count: u64,
}

impl DatasetCatalog {
    /// Builds a deterministic hierarchy sorted by [`ChunkId`].
    ///
    /// # Errors
    ///
    /// Descriptors must have unique identities, roots at level zero and every
    /// child must reference an existing descriptor exactly one level above it.
    pub fn new(
        dataset: DatasetId,
        mut descriptors: Vec<ChunkDescriptor>,
    ) -> Result<Self, DatasetError> {
        let Ok(chunk_count) = u32::try_from(descriptors.len()) else {
            return Err(DatasetError::CatalogTooLarge);
        };
        descriptors.sort_unstable_by_key(|descriptor| descriptor.id);
        validate_unique(&descriptors)?;
        let mut roots = Vec::new();
        let mut parent_indices = Vec::with_capacity(descriptors.len());
        for (index, descriptor) in descriptors.iter().enumerate() {
            let parent = parent_index(&descriptors, descriptor)?;
            if parent.is_none() {
                let Ok(index) = u32::try_from(index) else {
                    return Err(DatasetError::CatalogTooLarge);
                };
                roots.push(index);
            }
            parent_indices.push(parent);
        }
        if roots.is_empty() {
            return Err(DatasetError::MissingRoot);
        }
        let (child_ranges, children) = build_children(chunk_count, &parent_indices)?;
        Ok(Self {
            dataset,
            descriptors,
            roots,
            child_ranges,
            children,
            provider: None,
            logical_chunk_count: u64::from(chunk_count),
        })
    }

    /// Adapts compact `pdbiox` metadata without enumerating logical chunks.
    ///
    /// Provider chunks are validated and materialized independently by
    /// [`crate::ProviderDatasetBridge`]. Consequently, root and child
    /// traversal reports only descriptors explicitly stored by [`Self::new`].
    #[must_use]
    pub fn from_provider(descriptor: pdbiox::DatasetDescriptor) -> Self {
        Self {
            dataset: DatasetId::new(descriptor.id().get()),
            descriptors: Vec::new(),
            roots: Vec::new(),
            child_ranges: Vec::new(),
            children: Vec::new(),
            provider: Some(descriptor),
            logical_chunk_count: descriptor.chunk_count(),
        }
    }

    /// Stable dataset identity shared by every descriptor.
    #[must_use]
    pub const fn dataset_id(&self) -> DatasetId {
        self.dataset
    }

    /// Number of catalog descriptors, independent of residency.
    #[must_use]
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Number of logical chunks without forcing provider metadata expansion.
    #[must_use]
    pub fn logical_chunk_count(&self) -> u64 {
        self.logical_chunk_count
    }

    /// Whether the catalog contains no descriptors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.logical_chunk_count() == 0
    }

    /// Finds one descriptor in `O(log chunks)`.
    #[must_use]
    pub fn get(&self, id: ChunkId) -> Option<&ChunkDescriptor> {
        let index = self
            .descriptors
            .binary_search_by_key(&id, |descriptor| descriptor.id)
            .ok()?;
        self.descriptors.get(index)
    }

    /// Traversal roots in stable identity order, without allocating.
    #[must_use]
    pub fn roots(&self) -> CatalogRoots<'_> {
        CatalogRoots {
            catalog: self,
            indices: self.roots.iter(),
        }
    }

    /// Immediate children in stable identity order, without allocating.
    #[must_use]
    pub fn children(&self, parent: ChunkId) -> CatalogChildren<'_> {
        let indices = match self.descriptor_index(parent) {
            Some(index) => self.child_slice(index),
            None => &[],
        };
        CatalogChildren {
            catalog: self,
            indices: indices.iter(),
        }
    }

    /// Checked component-wise sum of all descriptor footprints.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::FootprintOverflow`] when any component sum
    /// exceeds `u64::MAX`.
    pub fn total_footprint(&self) -> Result<ChunkFootprint, DatasetError> {
        if self.provider.is_some() {
            return Err(DatasetError::ProviderFootprintUnavailable);
        }
        self.descriptors
            .iter()
            .try_fold(ChunkFootprint::default(), |total, descriptor| {
                total.checked_add(descriptor.footprint)
            })
    }

    fn descriptor_index(&self, id: ChunkId) -> Option<usize> {
        self.descriptors
            .binary_search_by_key(&id, |descriptor| descriptor.id)
            .ok()
    }

    fn child_slice(&self, parent: usize) -> &[u32] {
        let Some(range) = self.child_ranges.get(parent) else {
            return &[];
        };
        let start = range.start as usize;
        let end = start.saturating_add(range.count as usize);
        match self.children.get(start..end) {
            Some(children) => children,
            None => &[],
        }
    }
}

/// Iterator over catalog roots.
#[derive(Debug)]
pub struct CatalogRoots<'a> {
    catalog: &'a DatasetCatalog,
    indices: core::slice::Iter<'a, u32>,
}

impl<'a> Iterator for CatalogRoots<'a> {
    type Item = &'a ChunkDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        descriptor_at(self.catalog, self.indices.next().copied())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl ExactSizeIterator for CatalogRoots<'_> {}

/// Iterator over one descriptor's immediate children.
#[derive(Debug)]
pub struct CatalogChildren<'a> {
    catalog: &'a DatasetCatalog,
    indices: core::slice::Iter<'a, u32>,
}

impl<'a> Iterator for CatalogChildren<'a> {
    type Item = &'a ChunkDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        descriptor_at(self.catalog, self.indices.next().copied())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl ExactSizeIterator for CatalogChildren<'_> {}

fn descriptor_at(catalog: &DatasetCatalog, index: Option<u32>) -> Option<&ChunkDescriptor> {
    let index = index? as usize;
    catalog.descriptors.get(index)
}

fn validate_unique(descriptors: &[ChunkDescriptor]) -> Result<(), DatasetError> {
    if let Some(pair) = descriptors.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(DatasetError::DuplicateChunk { chunk: pair[0].id });
    }
    Ok(())
}

fn parent_index(
    descriptors: &[ChunkDescriptor],
    descriptor: &ChunkDescriptor,
) -> Result<Option<u32>, DatasetError> {
    let Some(parent_id) = descriptor.parent else {
        return if descriptor.level == 0 {
            Ok(None)
        } else {
            Err(DatasetError::InvalidHierarchyLevel {
                chunk: descriptor.id,
            })
        };
    };
    let Ok(parent) = descriptors.binary_search_by_key(&parent_id, |value| value.id) else {
        return Err(DatasetError::MissingParent {
            chunk: descriptor.id,
            parent: parent_id,
        });
    };
    if descriptor.level == 0 || descriptors[parent].level.checked_add(1) != Some(descriptor.level) {
        return Err(DatasetError::InvalidHierarchyLevel {
            chunk: descriptor.id,
        });
    }
    let Ok(parent) = u32::try_from(parent) else {
        return Err(DatasetError::CatalogTooLarge);
    };
    Ok(Some(parent))
}

fn build_children(
    chunk_count: u32,
    parents: &[Option<u32>],
) -> Result<(Vec<ChildRange>, Vec<u32>), DatasetError> {
    let mut counts = vec![0u32; chunk_count as usize];
    for parent in parents.iter().flatten() {
        let Some(count) = counts.get_mut(*parent as usize) else {
            return Err(DatasetError::CatalogTooLarge);
        };
        *count = count.checked_add(1).ok_or(DatasetError::CatalogTooLarge)?;
    }
    let mut offset = 0u32;
    let mut ranges = Vec::with_capacity(counts.len());
    for count in counts {
        ranges.push(ChildRange {
            start: offset,
            count,
        });
        offset = offset
            .checked_add(count)
            .ok_or(DatasetError::CatalogTooLarge)?;
    }
    let mut cursors = ranges.iter().map(|range| range.start).collect::<Vec<_>>();
    let mut children = vec![0u32; offset as usize];
    for (child, parent) in parents.iter().enumerate() {
        let Some(parent) = parent else { continue };
        let Some(cursor) = cursors.get_mut(*parent as usize) else {
            return Err(DatasetError::CatalogTooLarge);
        };
        let Some(slot) = children.get_mut(*cursor as usize) else {
            return Err(DatasetError::CatalogTooLarge);
        };
        *slot = u32::try_from(child).map_err(|_| DatasetError::CatalogTooLarge)?;
        *cursor = cursor.checked_add(1).ok_or(DatasetError::CatalogTooLarge)?;
    }
    Ok((ranges, children))
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
