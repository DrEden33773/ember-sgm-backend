use std::{ops::BitOr, sync::Arc};

use crate::{
  schemas::{PatternEdge, PatternVertex, PlanData, STR_TUPLE_SPLITTER, Vid, VidRef},
  utils::{dyn_graph::DynGraph, expand_graph::ExpandGraph},
};
use buckets::{ABucket, CBucket, FBucket, TBucket};
use crossbeam_queue::SegQueue;
use dashmap::DashMap;
use hashbrown::HashMap;

pub mod buckets;
pub mod buckets_impl;

#[inline]
fn resolve_var_name(target_var: &str) -> &str {
  target_var.split(STR_TUPLE_SPLITTER).nth(1).unwrap()
}

#[derive(Debug, Default)]
pub struct MatchingCtx {
  pub(crate) plan_data: Arc<PlanData>,

  pub(crate) f_block: DashMap<Vid, FBucket>,
  pub(crate) a_block: DashMap<Vid, ABucket>,
  pub(crate) c_block: DashMap<Vid, CBucket>,
  pub(crate) t_block: DashMap<Vid, TBucket>,

  pub(crate) grouped_partial_matches: SegQueue<Vec<DynGraph>>,
}

impl Clone for MatchingCtx {
  fn clone(&self) -> Self {
    Self {
      plan_data: self.plan_data.clone(),
      f_block: self.f_block.clone(),
      a_block: self.a_block.clone(),
      c_block: self.c_block.clone(),
      t_block: self.t_block.clone(),
      grouped_partial_matches: SegQueue::new(),
    }
  }
}

impl BitOr for MatchingCtx {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    let mut merged = self;

    merged.f_block.extend(rhs.f_block);
    merged.a_block.extend(rhs.a_block);
    merged.c_block.extend(rhs.c_block);
    merged.t_block.extend(rhs.t_block);

    // Note that `grouped_partial_matches` is not merged here,
    // because it will be only modified in the last `Report` step.

    merged
  }
}

impl MatchingCtx {
  pub fn new(plan_data: Arc<PlanData>) -> Self {
    Self {
      plan_data: plan_data.clone(),
      ..Default::default()
    }
  }
}

impl MatchingCtx {
  pub fn pattern_vs(&self) -> &HashMap<Vid, PatternVertex> {
    &self.plan_data.pattern_vs
  }

  pub fn pattern_es(&self) -> &HashMap<Vid, PatternEdge> {
    &self.plan_data.pattern_es
  }
}

impl MatchingCtx {
  #[inline]
  pub fn get_pattern_v(&self, vid: VidRef) -> Option<&PatternVertex> {
    self.plan_data.pattern_vs.get(vid)
  }

  #[inline]
  pub fn fetch_pattern_v_batch<'a>(
    &'a self,
    vid: impl IntoIterator<Item = VidRef<'a>>,
  ) -> Vec<PatternVertex> {
    vid
      .into_iter()
      .filter_map(|v| self.plan_data.pattern_vs.get(v))
      .cloned()
      .collect()
  }

  #[inline]
  pub fn get_pattern_e(&self, eid: VidRef) -> Option<&PatternEdge> {
    self.plan_data.pattern_es.get(eid)
  }

  #[inline]
  pub fn fetch_pattern_e_batch<'a>(
    &'a self,
    eid: impl IntoIterator<Item = VidRef<'a>>,
  ) -> Vec<PatternEdge> {
    eid
      .into_iter()
      .filter_map(|e| self.plan_data.pattern_es.get(e))
      .cloned()
      .collect()
  }
}

impl MatchingCtx {
  /// `Init`: Update `f_block` with matched_graph and frontier_vid
  pub fn append_to_f_block(
    &self,
    target_var: impl AsRef<str>,
    matched_graph: DynGraph,
    frontier_vid: VidRef,
  ) {
    let key = resolve_var_name(target_var.as_ref());

    // try to init f_block if key doesn't exist
    self.f_block.entry(key.to_string()).or_default();

    let next_idx = self.f_block.get(key).unwrap().all_matched.len();
    let mut f_bucket = self.f_block.get_mut(key).unwrap();

    f_bucket.all_matched.push(matched_graph);
    f_bucket
      .matched_with_frontiers
      .entry(next_idx)
      .or_default()
      .push(frontier_vid.to_string());
  }

  /// `Foreach`: Update `f_block` with `f_bucket`
  pub fn update_f_block(&self, target_var: &str, f_bucket: FBucket) {
    let key = resolve_var_name(target_var);
    self.f_block.insert(key.to_string(), f_bucket);
  }

  /// `GetAdj`: Get `f_block` with `target_var` (pop it out)
  pub fn pop_from_f_block(&self, single_op: &str) -> Option<FBucket> {
    let key = resolve_var_name(single_op);
    self.f_block.remove(key).map(|f| f.1)
  }
}

impl MatchingCtx {
  /// `GetAdj`: Update `a_block` with `a_bucket`
  pub fn update_a_block(&self, target_var: &str, a_bucket: ABucket) {
    let key = resolve_var_name(target_var);
    self.a_block.insert(key.to_string(), a_bucket);
  }

  /// `Intersect(Ai)`: Get the `group` from `single_op` represented `a_bucket` by `pattern_str`
  ///
  /// (pop it out)
  pub fn pop_group_by_pat_from_a_block(
    &self,
    single_op: &str,
    curr_pat_str: &str,
  ) -> Option<Vec<ExpandGraph>> {
    let key = resolve_var_name(single_op);
    self
      .a_block
      .get_mut(key)?
      .next_pat_grouped_expanding
      .remove(curr_pat_str)
  }
}

impl MatchingCtx {
  /// `Intersect`: Update `c_block` with `c_bucket`
  pub fn update_c_block(&self, target_var: &str, c_bucket: CBucket) {
    let key = resolve_var_name(target_var);
    self.c_block.insert(key.to_string(), c_bucket);
  }

  /// `Foreach`: Get `c_block` with `single_op` (pop it out)
  pub fn pop_from_c_block(&self, single_op: &str) -> Option<CBucket> {
    let key = resolve_var_name(single_op);
    self.c_block.remove(key).map(|c| c.1)
  }
}

impl MatchingCtx {
  /// `Intersect`: Update `t_block` with `t_bucket`
  pub fn update_t_block(&self, target_var: &str, t_bucket: TBucket) {
    let key = resolve_var_name(target_var);
    self.t_block.insert(key.to_string(), t_bucket);
  }

  /// `Intersect(Tx)`: Get `t_block` with `single_op` (pop it out)
  pub fn pop_from_t_block(&self, single_op: &str) -> Option<TBucket> {
    let key = resolve_var_name(single_op);
    self.t_block.remove(key).map(|t| t.1)
  }
}
