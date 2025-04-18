use super::dyn_graph::DynGraph;
use crate::schemas::*;
use hashbrown::HashMap;
use indexmap::IndexMap;
use std::{cmp::Ordering, sync::Arc};

#[derive(Debug, Clone)]
pub struct ExpandGraph<VType: VBase = DataVertex, EType: EBase = DataEdge> {
  pub(crate) dyn_graph: Arc<DynGraph<VType, EType>>,

  pub(crate) pending_v_grouped_dangling_eids: IndexMap<Vid, Vec<Eid>>,
  pub(crate) target_vs: Vec<Vid>,

  pub(crate) dangling_e_entities: HashMap<Eid, EType>,
  pub(crate) target_v_entities: HashMap<Vid, VType>,

  pub(crate) dangling_e_patterns: HashMap<Eid, String>,
  pub(crate) target_v_patterns: HashMap<Vid, String>,
}

impl<VType: VBase, EType: EBase> Default for ExpandGraph<VType, EType> {
  fn default() -> Self {
    Self {
      dyn_graph: Default::default(),
      pending_v_grouped_dangling_eids: Default::default(),
      target_vs: Default::default(),
      dangling_e_entities: Default::default(),
      target_v_entities: Default::default(),
      dangling_e_patterns: Default::default(),
      target_v_patterns: Default::default(),
    }
  }
}

impl<VType: VBase, EType: EBase> From<Arc<DynGraph<VType, EType>>> for ExpandGraph<VType, EType> {
  fn from(dyn_graph: Arc<DynGraph<VType, EType>>) -> Self {
    Self {
      dyn_graph,
      ..Default::default()
    }
  }
}

impl<VType: VBase, EType: EBase> From<ExpandGraph<VType, EType>> for DynGraph<VType, EType> {
  fn from(mut val: ExpandGraph<VType, EType>) -> Self {
    let mut graph = (*val.dyn_graph).clone();

    graph.update_v_batch(val.target_v_entities.into_values().map(|v| {
      let pattern = val.target_v_patterns.remove(v.vid()).unwrap();
      (v, pattern)
    }));

    for target_v in val.target_vs {
      let dangling_eids = val.pending_v_grouped_dangling_eids.get(&target_v).unwrap();

      let dangling_e_pattern_pairs = dangling_eids
        .iter()
        .filter_map(|eid| val.dangling_e_entities.remove(eid))
        .map(|e| {
          let pattern = val.dangling_e_patterns.remove(e.eid()).unwrap();
          (e, pattern)
        });
      graph.update_e_batch(dangling_e_pattern_pairs);
    }

    graph
  }
}

impl<VType: VBase, EType: EBase> ExpandGraph<VType, EType> {
  /// Update valid dangling edges and return them
  pub fn update_valid_dangling_edges<'a>(
    &'a mut self,
    dangling_edge_pattern_pairs: impl IntoIterator<Item = (&'a EType, &'a str)>,
  ) {
    for (edge, pattern) in dangling_edge_pattern_pairs {
      if self.dyn_graph.has_eid(edge.eid()) {
        continue;
      }

      match self.dyn_graph.pick_e_connective_vid(edge) {
        (Some(_), Some(_)) | (None, None) => continue,
        (Some(_src_vid), None) => {
          // `src_vid` is connected, `dst_vid` is pending
          self
            .pending_v_grouped_dangling_eids
            .entry(edge.dst_vid().to_string())
            .or_default()
            .push(edge.eid().to_string());
        }
        (None, Some(_dst_vid)) => {
          // `dst_vid` is connected, `src_vid` is pending
          self
            .pending_v_grouped_dangling_eids
            .entry(edge.src_vid().to_string())
            .or_default()
            .push(edge.eid().to_string());
        }
      }

      self
        .dangling_e_patterns
        .insert(edge.eid().to_string(), pattern.to_string());

      self
        .dangling_e_entities
        .insert(edge.eid().to_string(), edge.clone());
    }
  }

  /// Sort the pending_v_grouped_dangling_eids by the key
  ///
  /// - Note that this function should only be called after `update_valid_target_vertices`
  pub fn sort_key_after_update_valid_target_vertices(&mut self) {
    self.pending_v_grouped_dangling_eids.sort_unstable_keys();
  }

  /// Update `valid target vertices` and return them
  ///
  /// - Vertices of any `dangling_edge` could be added to `target_v_adj_table`
  pub fn update_valid_target_vertices(
    &mut self,
    asc_target_vertex_pattern_pairs: &[(VType, String)],
  ) -> Vec<String> {
    // Skip if either collection is empty
    if asc_target_vertex_pattern_pairs.is_empty() || self.pending_v_grouped_dangling_eids.is_empty()
    {
      return vec![];
    }

    let pending_v_grouped_dangling_eids = self.pending_v_grouped_dangling_eids.as_slice();

    // Use index-based approach instead of iterators for better debug mode performance
    let pending_len = self.pending_v_grouped_dangling_eids.len();
    let vertex_len = asc_target_vertex_pattern_pairs.len();
    self.target_vs.reserve(pending_len.min(vertex_len));

    let mut legal_vids = Vec::with_capacity(vertex_len.min(pending_len));

    let mut pending_idx = 0;
    let mut vertex_idx = 0;

    // Continue until one of the collections is exhausted
    while pending_idx < pending_len && vertex_idx < vertex_len {
      // Get current elements by index
      let (pending_vid, _) = pending_v_grouped_dangling_eids
        .get_index(pending_idx)
        .unwrap();
      let (vertex, pattern) = &asc_target_vertex_pattern_pairs[vertex_idx];

      // Skip if the vertex is already in the graph
      if self.dyn_graph.has_vid(vertex.vid()) {
        vertex_idx += 1;
        continue;
      }

      // Compare the pending_vid and vertex.vid()
      match pending_vid.as_str().cmp(vertex.vid()) {
        Ordering::Equal => {
          // Found a match, add the vertex to the `target_vs`
          self.target_vs.push(vertex.vid().to_string());

          // Update other information
          legal_vids.push(vertex.vid().to_string());

          self
            .target_v_patterns
            .insert(vertex.vid().to_string(), pattern.clone());

          self
            .target_v_entities
            .insert(vertex.vid().to_string(), vertex.clone());

          // Move both indices
          pending_idx += 1;
          vertex_idx += 1;
        }
        Ordering::Less => {
          // pending_vid < vertex.vid(), move pending index
          pending_idx += 1;
        }
        Ordering::Greater => {
          // pending_vid > vertex.vid(), current vertex is not found in pending,
          // move to next vertex
          vertex_idx += 1;
        }
      }
    }

    legal_vids
  }
}

impl<VType: VBase, EType: EBase> ExpandGraph<VType, EType> {
  #[inline]
  pub fn has_common_pending_v(&self, other: &Self) -> bool {
    let (shorter, longer) =
      if self.pending_v_grouped_dangling_eids.len() < other.pending_v_grouped_dangling_eids.len() {
        (self, other)
      } else {
        (other, self)
      };

    shorter
      .pending_v_grouped_dangling_eids
      .iter()
      .any(|(vid, _)| longer.pending_v_grouped_dangling_eids.contains_key(vid))
  }
}

/// 1. Take two expand_graphs' `vertices` and `non-dangling-edges` into a new graph
/// 2. Iterate through the `dangling_edges` of both, select those connective ones
pub fn union_then_intersect_on_connective_v<VType: VBase, EType: EBase>(
  l_expand_graph: &ExpandGraph<VType, EType>,
  r_expand_graph: &ExpandGraph<VType, EType>,
) -> Vec<ExpandGraph<VType, EType>> {
  let grouped_l = l_expand_graph.pending_v_grouped_dangling_eids.as_slice();
  let grouped_r = r_expand_graph.pending_v_grouped_dangling_eids.as_slice();

  if grouped_l.is_empty() || grouped_r.is_empty() {
    return vec![];
  }

  // Create the basic graph structure (only create once)
  let new_graph = (*l_expand_graph.dyn_graph).clone() | (*r_expand_graph.dyn_graph).clone();
  let new_graph = Arc::new(new_graph);

  // Use index-based approach instead of iterators for better debug mode performance
  let l_len = grouped_l.len();
  let r_len = grouped_r.len();

  let mut result = Vec::with_capacity(l_len.min(r_len));

  let mut l_idx = 0;
  let mut r_idx = 0;

  // Continue iterating as long as both collections have unprocessed elements
  while l_idx < l_len && r_idx < r_len {
    // Get current elements by index
    let (l_vid, l_dangling_eids) = grouped_l.get_index(l_idx).unwrap();
    let (r_vid, r_dangling_eids) = grouped_r.get_index(r_idx).unwrap();

    match l_vid.cmp(r_vid) {
      Ordering::Equal => {
        // Found a common vertex, process it directly
        let mut expanding_dg: ExpandGraph<VType, EType> = new_graph.clone().into();

        expanding_dg.update_valid_dangling_edges(l_dangling_eids.iter().map(|eid| {
          (
            &l_expand_graph.dangling_e_entities[eid],
            l_expand_graph.dangling_e_patterns[eid].as_str(),
          )
        }));
        expanding_dg.update_valid_dangling_edges(r_dangling_eids.iter().map(|eid| {
          (
            &r_expand_graph.dangling_e_entities[eid],
            r_expand_graph.dangling_e_patterns[eid].as_str(),
          )
        }));
        expanding_dg.sort_key_after_update_valid_target_vertices();

        result.push(expanding_dg);

        // Move both indices
        l_idx += 1;
        r_idx += 1;
      }
      Ordering::Less => {
        // l_vid < r_vid, move the left index
        l_idx += 1;
      }
      Ordering::Greater => {
        // l_vid > r_vid, move the right index
        r_idx += 1;
      }
    }
  }

  result
}
