use crate::schemas::*;
use hashbrown::{HashMap, HashSet};
use std::{
  hash::Hash,
  ops::{BitOr, BitOrAssign},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VNode {
  pub(crate) e_in: HashSet<Eid>,
  pub(crate) e_out: HashSet<Eid>,
}

impl Hash for VNode {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.e_in.iter().for_each(|eid| eid.hash(state));
    self.e_out.iter().for_each(|eid| eid.hash(state));
  }
}

impl BitOrAssign for VNode {
  fn bitor_assign(&mut self, rhs: Self) {
    self.e_in.extend(rhs.e_in);
    self.e_out.extend(rhs.e_out);
  }
}

impl BitOr for VNode {
  type Output = VNode;

  fn bitor(self, rhs: Self) -> Self::Output {
    let mut e_in = self.e_in;
    let mut e_out = self.e_out;
    e_in.extend(rhs.e_in);
    e_out.extend(rhs.e_out);
    VNode { e_in, e_out }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynGraph<VType: VBase = DataVertex, EType: EBase = DataEdge> {
  /// vid -> v_entity
  pub(crate) v_entities: HashMap<Vid, VType>,

  /// eid -> e_entity
  pub(crate) e_entities: HashMap<Eid, EType>,

  pub(crate) adj_table: HashMap<Vid, VNode>,

  /// vid -> v_pattern_str
  pub(crate) v_patterns: HashMap<Vid, String>,

  /// eid -> e_pattern_str
  pub(crate) e_patterns: HashMap<Eid, String>,
}

impl<VType: VBase, EType: EBase> Hash for DynGraph<VType, EType> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    for (vid, v_node) in &self.adj_table {
      vid.hash(state);
      v_node.hash(state);
    }
  }
}

impl<VType: VBase, EType: EBase> AsRef<Self> for DynGraph<VType, EType> {
  fn as_ref(&self) -> &Self {
    self
  }
}

impl<VType: VBase, EType: EBase> Default for DynGraph<VType, EType> {
  fn default() -> Self {
    Self {
      v_entities: Default::default(),
      e_entities: Default::default(),
      adj_table: Default::default(),
      v_patterns: Default::default(),
      e_patterns: Default::default(),
    }
  }
}

impl<VType: VBase, EType: EBase> BitOr for DynGraph<VType, EType> {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    let mut v_entities = self.v_entities;
    let mut e_entities = self.e_entities;
    let mut v_patterns = self.v_patterns;
    let mut e_patterns = self.e_patterns;
    v_entities.extend(rhs.v_entities);
    e_entities.extend(rhs.e_entities);
    v_patterns.extend(rhs.v_patterns);
    e_patterns.extend(rhs.e_patterns);

    let mut res = DynGraph {
      v_entities,
      e_entities,
      v_patterns,
      e_patterns,
      ..Default::default()
    };

    for (vid, v_node) in self.adj_table {
      res.adj_table.insert(vid, v_node);
    }
    for (vid, v_node) in rhs.adj_table {
      if res.adj_table.contains_key(&vid) {
        res.adj_table.get_mut(&vid).unwrap().bitor_assign(v_node);
      } else {
        res.adj_table.insert(vid, v_node);
      }
    }
    res
  }
}

impl<VType: VBase, EType: EBase> DynGraph<VType, EType> {
  pub fn is_subset_of(&self, other: &Self) -> bool {
    // adj_table
    for (vid, v_node) in self.adj_table.iter() {
      // vertex
      if !other.adj_table.contains_key(vid) {
        return false;
      }
      // in_edge
      if !v_node.e_in.is_subset(&other.adj_table[vid].e_in) {
        return false;
      }
      // out_edge
      if !v_node.e_out.is_subset(&other.adj_table[vid].e_out) {
        return false;
      }
    }
    // v_patterns
    for (vid, v_pattern) in self.v_patterns.iter() {
      if !other.v_patterns.contains_key(vid) {
        return false;
      }
      if other.v_patterns[vid] != *v_pattern {
        return false;
      }
    }
    // e_patterns
    for (eid, e_pattern) in self.e_patterns.iter() {
      if !other.e_patterns.contains_key(eid) {
        return false;
      }
      if other.e_patterns[eid] != *e_pattern {
        return false;
      }
    }
    true
  }

  pub fn is_superset_of(&self, other: &Self) -> bool {
    other.is_subset_of(self)
  }
}

impl<VType: VBase, EType: EBase> DynGraph<VType, EType> {
  pub fn update_v(&mut self, vertex: VType, pattern: String) -> &mut Self {
    let vid = vertex.vid().to_owned();
    self.v_entities.insert(vid.to_owned(), vertex);

    // self.adj_table.insert(vid.to_owned(), VNode::default()); // BUG
    self.adj_table.entry(vid.to_owned()).or_default();

    self.v_patterns.insert(vid, pattern);
    self
  }

  pub fn update_v_batch(
    &mut self,
    v_pattern_pairs: impl IntoIterator<Item = (VType, String)>,
  ) -> &mut Self {
    for (vertex, pattern) in v_pattern_pairs {
      self.update_v(vertex, pattern);
    }
    self
  }

  pub fn update_e(&mut self, edge: EType, pattern: String) -> &mut Self {
    let eid = edge.eid().to_owned();
    let src_vid = edge.src_vid().to_owned();
    let dst_vid = edge.dst_vid().to_owned();

    if self.has_all_vids(&[&src_vid, &dst_vid]) {
      self.e_entities.insert(eid.to_owned(), edge);

      self
        .adj_table
        .entry(src_vid)
        .or_default()
        .e_out
        .insert(eid.to_owned());

      self
        .adj_table
        .entry(dst_vid)
        .or_default()
        .e_in
        .insert(eid.to_owned());

      self.e_patterns.insert(eid, pattern);

      self
    } else if self.has_vid(&src_vid) {
      panic!(
        "Detected `half-dangling edge`:\n\t(vid: {}) -[eid: {}]-> ?",
        src_vid, eid
      );
    } else if self.has_vid(&dst_vid) {
      panic!(
        "Detected `half-dangling edge`:\n\t? -[eid: {}]-> (vid: {})",
        eid, dst_vid
      );
    } else {
      panic!("Detected `dangling edge`:\n\t? -[eid: {}]-> ?", eid);
    }
  }

  pub fn update_e_batch(
    &mut self,
    e_pattern_pairs: impl IntoIterator<Item = (EType, String)>,
  ) -> &mut Self {
    for (edge, pattern) in e_pattern_pairs {
      self.update_e(edge, pattern);
    }
    self
  }

  pub fn remove_e(&mut self, eid: EidRef) -> &mut Self {
    if !self.has_eid(eid) {
      return self;
    }

    for v in self.adj_table.values_mut() {
      v.e_in.remove(eid);
      v.e_out.remove(eid);
    }
    self.e_entities.remove(eid);

    self
  }

  pub fn remove_e_batch(&mut self, eids: &[EidRef]) -> &mut Self {
    for eid in eids {
      self.remove_e(eid);
    }
    self
  }
}

impl<VType: VBase, EType: EBase> DynGraph<VType, EType> {
  #[inline]
  pub fn get_v_from_vid(&self, vid: VidRef) -> Option<&VType> {
    self.v_entities.get(vid)
  }
  #[inline]
  pub fn get_e_from_eid(&self, eid: EidRef) -> Option<&EType> {
    self.e_entities.get(eid)
  }

  pub fn get_first_connective_vid_for_e(&self, edge: &EType) -> Option<Vid> {
    let src_vid = edge.src_vid();
    let dst_vid = edge.dst_vid();
    if self.has_vid(src_vid) {
      Some(src_vid.to_owned())
    } else if self.has_vid(dst_vid) {
      Some(dst_vid.to_owned())
    } else {
      None
    }
  }

  #[inline]
  pub fn get_vid_set(&self) -> HashSet<Vid> {
    self.v_entities.keys().cloned().collect()
  }
  #[inline]
  pub fn get_eid_set(&self) -> HashSet<Eid> {
    self.e_entities.keys().cloned().collect()
  }
  #[inline]
  pub fn get_v_entities(&self) -> Vec<VType> {
    self.v_entities.values().cloned().collect()
  }
  #[inline]
  pub fn get_e_entities(&self) -> Vec<EType> {
    self.e_entities.values().cloned().collect()
  }
  #[inline]
  pub fn get_v_pat_str_set(&self) -> HashSet<String> {
    self.v_patterns.values().cloned().collect()
  }
  #[inline]
  pub fn get_e_pat_str_set(&self) -> HashSet<String> {
    self.e_patterns.values().cloned().collect()
  }
  #[inline]
  pub fn get_all_pat_str_set(&self) -> HashSet<String> {
    let mut res = self.get_v_pat_str_set();
    res.extend(self.get_e_pat_str_set());
    res
  }
  #[inline]
  pub fn get_v_pattern_pairs(&self) -> Vec<(VType, String)> {
    self
      .v_entities
      .iter()
      .map(|(vid, v_entity)| {
        let pattern = self.v_patterns[vid].to_owned();
        (v_entity.clone(), pattern)
      })
      .collect()
  }
  #[inline]
  pub fn get_e_pattern_pairs(&self) -> Vec<(EType, String)> {
    self
      .e_entities
      .iter()
      .map(|(eid, e_entity)| {
        let pattern = self.e_patterns[eid].to_owned();
        (e_entity.clone(), pattern)
      })
      .collect()
  }
  #[inline]
  pub fn get_v_count(&self) -> usize {
    self.v_entities.len()
  }
  #[inline]
  pub fn get_e_count(&self) -> usize {
    self.e_entities.len()
  }
}

impl<VType: VBase, EType: EBase> DynGraph<VType, EType> {
  #[inline]
  pub fn has_vid(&self, vid: VidRef) -> bool {
    self.v_entities.contains_key(vid)
  }
  #[inline]
  pub fn has_all_vids(&self, vids: &[VidRef]) -> bool {
    vids.iter().all(|vid| self.has_vid(vid))
  }
  #[inline]
  pub fn has_any_vids(&self, vids: &[VidRef]) -> bool {
    vids.iter().any(|vid| self.has_vid(vid))
  }

  #[inline]
  pub fn has_eid(&self, eid: EidRef) -> bool {
    self.e_entities.contains_key(eid)
  }
  #[inline]
  pub fn has_all_eids(&self, eids: &[EidRef]) -> bool {
    eids.iter().all(|eid| self.has_eid(eid))
  }
  #[inline]
  pub fn has_any_eids(&self, eids: &[EidRef]) -> bool {
    eids.iter().any(|eid| self.has_eid(eid))
  }
}

impl<VType: VBase, EType: EBase> DynGraph<VType, EType> {
  #[inline]
  pub fn is_e_connective(&self, edge: &EType) -> bool {
    self.has_any_vids(&[edge.src_vid(), edge.dst_vid()])
  }
  #[inline]
  pub fn is_e_full_connective(&self, edge: &EType) -> bool {
    self.has_all_vids(&[edge.src_vid(), edge.dst_vid()])
  }
}
