use super::dyn_graph::{DynGraph, VNode};
use crate::schemas::*;
use hashbrown::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandGraph<VType: VBase = DataVertex, EType: EBase = DataEdge> {
  pub(crate) dyn_graph: DynGraph<VType, EType>,
  pub(crate) target_v_adj_table: HashMap<Vid, VNode>,

  pub(crate) dangling_e_entities: HashMap<Eid, EType>,
  pub(crate) target_v_entities: HashMap<Vid, VType>,

  pub(crate) dangling_e_patterns: HashMap<Eid, String>,
  pub(crate) target_v_patterns: HashMap<Vid, String>,
}

impl<VType: VBase, EType: EBase> Default for ExpandGraph<VType, EType> {
  fn default() -> Self {
    Self {
      dyn_graph: Default::default(),
      target_v_adj_table: Default::default(),
      dangling_e_entities: Default::default(),
      target_v_entities: Default::default(),
      dangling_e_patterns: Default::default(),
      target_v_patterns: Default::default(),
    }
  }
}

impl<VType: VBase, EType: EBase> From<&DynGraph<VType, EType>> for ExpandGraph<VType, EType> {
  fn from(val: &DynGraph<VType, EType>) -> Self {
    Self {
      dyn_graph: val.clone(),
      ..Default::default()
    }
  }
}

impl<VType: VBase, EType: EBase> From<ExpandGraph<VType, EType>> for DynGraph<VType, EType> {
  fn from(mut val: ExpandGraph<VType, EType>) -> Self {
    let mut graph = val.dyn_graph;

    graph.update_v_batch(val.target_v_entities.into_values().map(|v| {
      let pattern = val.target_v_patterns.remove(v.vid()).unwrap();
      (v, pattern)
    }));

    for target_v in val.target_v_adj_table.keys() {
      let mut dangling_eids = val.target_v_adj_table[target_v].e_out.to_owned();
      dangling_eids.extend(val.target_v_adj_table[target_v].e_in.clone());

      let dangling_e_pattern_pairs = dangling_eids
        .into_iter()
        .filter_map(|eid| val.dangling_e_entities.remove(&eid))
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
  pub fn get_vid_set(&self) -> HashSet<String> {
    self.dyn_graph.get_vid_set()
  }
  pub fn get_eid_set(&self) -> HashSet<String> {
    self.dyn_graph.get_eid_set()
  }
  pub fn get_v_count(&self) -> usize {
    self.dyn_graph.get_v_count()
  }
  pub fn get_e_count(&self) -> usize {
    self.dyn_graph.get_e_count()
  }
  pub fn dangling_e_patterns(&self) -> &HashMap<Eid, String> {
    &self.dangling_e_patterns
  }
  pub fn target_v_patterns(&self) -> &HashMap<Vid, String> {
    &self.target_v_patterns
  }
}

impl<VType: VBase, EType: EBase> ExpandGraph<VType, EType> {
  /// Group dangling edges by their pending(unconnected) vertices.
  pub fn group_dangling_e_by_pending_v(&self) -> HashMap<String, Vec<EType>> {
    let mut grouped: HashMap<String, Vec<EType>> = HashMap::new();

    for dangling_e in self.dangling_e_entities.values() {
      if self.dyn_graph.has_vid(dangling_e.src_vid()) {
        grouped
          // .entry(dangling_e.src_vid().to_owned()) // BUG
          .entry(dangling_e.dst_vid().to_owned())
          .or_default()
          .push(dangling_e.clone());
      } else if self.dyn_graph.has_vid(dangling_e.dst_vid()) {
        grouped
          // .entry(dangling_e.dst_vid().to_owned()) // BUG
          .entry(dangling_e.src_vid().to_owned())
          .or_default()
          .push(dangling_e.clone());
      }
    }

    grouped
  }

  /// Check if the edge is a valid dangling edge
  fn is_valid_dangling_edge(&self, e: &EType) -> bool {
    self.dyn_graph.is_e_connective(e)
      && !self.dyn_graph.is_e_full_connective(e)
      && !self.dyn_graph.has_eid(e.eid())
  }

  /// Update valid dangling edges and return them
  pub fn update_valid_dangling_edges<'a>(
    &'a mut self,
    dangling_edges: impl IntoIterator<Item = (&'a EType, &'a str)>,
  ) -> HashSet<String> {
    let mut legal_eids = HashSet::new();

    for (e, pattern) in dangling_edges {
      if !self.is_valid_dangling_edge(e) {
        continue;
      }
      legal_eids.insert(e.eid().to_owned());
      self
        .dangling_e_patterns
        .insert(e.eid().to_owned(), pattern.to_owned());
      self
        .dangling_e_entities
        .insert(e.eid().to_owned(), e.to_owned());
    }

    legal_eids
  }

  /// Check if the vertex is a valid target vertex
  fn is_valid_target(&self, v: &VType) -> bool {
    for e in self.dangling_e_entities.values() {
      if e.contains(v.vid()) && !self.dyn_graph.has_vid(v.vid()) {
        return true;
      }
    }
    false
  }

  /// Update valid target vertices and return them
  ///
  /// - Vertices of any `dangling_edge` could be added to `target_v_adj_table`
  pub fn update_valid_target_vertices<'a>(
    &'a mut self,
    target_vertices: impl IntoIterator<Item = (&'a VType, &'a str)>,
  ) -> HashSet<String> {
    let mut legal_vids = HashSet::new();
    for (v, pattern) in target_vertices {
      if !self.is_valid_target(v) {
        continue;
      }
      legal_vids.insert(v.vid().to_owned());
      self
        .target_v_patterns
        .insert(v.vid().to_owned(), pattern.to_owned());
      self
        .target_v_entities
        .insert(v.vid().to_owned(), v.to_owned());
    }

    for dangling_e in self.dangling_e_entities.keys() {
      let e = &self.dangling_e_entities[dangling_e];
      if self.target_v_entities.contains_key(e.src_vid()) {
        self
          .target_v_adj_table
          .entry(e.src_vid().to_owned())
          .or_default()
          .e_out
          .insert(e.eid().to_owned());
      }
      if self.target_v_entities.contains_key(e.dst_vid()) {
        self
          .target_v_adj_table
          .entry(e.dst_vid().to_owned())
          .or_default()
          .e_in
          .insert(e.eid().to_owned());
      }
    }
    legal_vids
  }
}

pub fn union_then_intersect_on_connective_v<VType: VBase, EType: EBase>(
  l_expand_graph: &ExpandGraph<VType, EType>,
  r_expand_graph: &ExpandGraph<VType, EType>,
) -> Vec<ExpandGraph<VType, EType>> {
  let l_graph = &l_expand_graph.dyn_graph;
  let r_graph = &r_expand_graph.dyn_graph;

  let l_v_pat_pairs = l_graph.get_v_pattern_pairs();
  let r_v_pat_pairs = r_graph.get_v_pattern_pairs();
  let l_e_pat_pairs = l_graph.get_e_pattern_pairs();
  let r_e_pat_pairs = r_graph.get_e_pattern_pairs();

  let mut new_graph = DynGraph::<VType, EType>::default();
  new_graph.update_v_batch(l_v_pat_pairs.into_iter().chain(r_v_pat_pairs));
  new_graph.update_e_batch(l_e_pat_pairs.into_iter().chain(r_e_pat_pairs));

  let (grouped_l, grouped_r) = (
    l_expand_graph.group_dangling_e_by_pending_v(),
    r_expand_graph.group_dangling_e_by_pending_v(),
  );

  let mut result = vec![];

  for (l_pending_vid, l_dangling_es) in &grouped_l {
    for (r_pending_vid, r_dangling_es) in &grouped_r {
      if l_pending_vid != r_pending_vid {
        continue;
      }

      let mut expanding_dg: ExpandGraph<VType, EType> = new_graph.as_ref().into();
      expanding_dg.update_valid_dangling_edges(
        l_dangling_es
          .iter()
          .map(|e| (e, l_expand_graph.dangling_e_patterns[e.eid()].as_str())),
      );
      expanding_dg.update_valid_dangling_edges(
        r_dangling_es
          .iter()
          .map(|e| (e, r_expand_graph.dangling_e_patterns[e.eid()].as_str())),
      );

      result.push(expanding_dg);
    }
  }

  result
}
