use crate::{
  schemas::Vid,
  utils::{dyn_graph::DynGraph, expand_graph::ExpandGraph},
};
use hashbrown::HashMap;

#[derive(Debug, Clone, Default)]
pub struct FBucket {
  pub(crate) all_matched: Vec<DynGraph>,
  pub(crate) matched_with_frontiers: HashMap<usize, Vec<Vid>>,
}

#[derive(Debug, Clone)]
pub struct ABucket {
  pub(crate) curr_pat_vid: Vid,
  pub(crate) all_matched: Vec<DynGraph>,
  pub(crate) matched_with_frontiers: HashMap<usize, Vec<Vid>>,
  pub(crate) next_pat_grouped_expanding: HashMap<Vid, Vec<ExpandGraph>>,
}
impl ABucket {
  pub fn new(curr_pat_vid: Vid) -> Self {
    Self {
      curr_pat_vid,
      all_matched: vec![],
      matched_with_frontiers: HashMap::new(),
      next_pat_grouped_expanding: HashMap::new(),
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct CBucket {
  pub(crate) all_expanded: Vec<ExpandGraph>,
  pub(crate) expanded_with_frontiers: HashMap<usize, Vec<Vid>>,
}

#[derive(Debug, Clone)]
pub struct TBucket {
  pub(crate) target_pat_vid: Vid,
  pub(crate) expanding_graphs: Vec<ExpandGraph>,
}

impl TBucket {
  pub fn new(target_pat_vid: Vid) -> Self {
    Self {
      target_pat_vid,
      expanding_graphs: vec![],
    }
  }
}
