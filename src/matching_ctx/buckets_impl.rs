use super::buckets::{ABucket, CBucket, FBucket, TBucket};
use crate::{
  schemas::{
    DataEdge, DataVertex, EBase, LabelRef, PatternAttr, PatternEdge, PatternVertex, Vid, VidRef,
  },
  storage::StorageAdapter,
  utils::{
    dyn_graph::DynGraph,
    expand_graph::{ExpandGraph, union_then_intersect_on_connective_v},
  },
};
use futures::future;
use hashbrown::{HashMap, HashSet};

async fn does_data_v_satisfy_pattern(
  dg_vid: VidRef<'_>,
  pat_vid: VidRef<'_>,
  pat_v_entities: &HashMap<Vid, PatternVertex>,
  storage_adapter: &impl StorageAdapter,
) -> bool {
  let pat_v = pat_v_entities.get(pat_vid).unwrap();
  let data_v = storage_adapter.get_v(dg_vid).await.unwrap();

  if pat_v.label != data_v.label {
    return false;
  }

  if pat_v.attr.is_none() {
    true
  } else if data_v.attrs.is_empty() {
    false
  } else {
    let pat_attr = pat_v.attr.as_ref().unwrap();
    if !data_v.attrs.contains_key(&pat_attr.key) {
      return false;
    }
    let data_value = data_v.attrs.get(&pat_attr.key).unwrap();
    pat_attr.op.operate_on(data_value, &pat_attr.value)
  }
}

impl FBucket {
  pub async fn from_c_bucket(c_bucket: &CBucket) -> Self {
    let mut all_matched = vec![];
    let mut matched_with_frontiers = HashMap::new();

    let all_expanded = &c_bucket.all_expanded;
    let expanded_with_pivots = &c_bucket.expanded_with_frontiers;

    for (idx, matched) in all_expanded.iter().enumerate() {
      all_matched.push(matched.to_owned().into());
      matched_with_frontiers
        .entry(idx)
        .or_insert_with(Vec::new)
        .extend(expanded_with_pivots[&idx].clone());
    }

    Self {
      all_matched,
      matched_with_frontiers,
    }
  }
}

impl ABucket {
  pub fn from_f_bucket(f_bucket: &FBucket, curr_pat_vid: VidRef) -> Self {
    Self {
      curr_pat_vid: curr_pat_vid.to_owned(),
      all_matched: f_bucket.all_matched.to_owned(),
      matched_with_frontiers: f_bucket.matched_with_frontiers.to_owned(),
      next_pat_grouped_expanding: HashMap::new(),
    }
  }

  pub async fn incremental_load_new_edges(
    &mut self,
    pattern_es: impl IntoIterator<Item = PatternEdge>,
    pattern_vs: &HashMap<Vid, PatternVertex>,
    storage_adapter: &impl StorageAdapter,
  ) -> HashSet<String> {
    let pattern_es = pattern_es.into_iter().collect::<Vec<_>>();
    let mut connected_data_vids = HashSet::new();

    // iter: `matched` data_graphs
    for (&idx, frontiers) in self.matched_with_frontiers.iter() {
      let matched_dg = &self.all_matched[idx];

      // iter: `frontier_vid` on current data_graph
      for frontier_vid in frontiers.iter() {
        let mut is_frontier_connected = false;

        // iter: `pattern_edges`
        for pat_e in pattern_es.iter() {
          if matched_dg.get_e_pat_str_set().contains(pat_e.eid()) {
            continue;
          }

          let label = pat_e.label();
          let attr = pat_e.attr.as_ref();
          let mut next_vid_grouped_conn_es = HashMap::new();
          let mut next_vid_grouped_conn_pat_strs = HashMap::new();
          let next_pat_vid;

          let is_matched_data_es_empty = if self.curr_pat_vid == pat_e.src_vid() {
            next_pat_vid = pat_e.dst_vid();

            let matched_data_es = incremental_match(LoadWithCondCtx {
              pattern_vs,
              storage_adapter,
              curr_matched_dg: matched_dg,
              frontier_vid,
              curr_pat_e: pat_e,
              e_label: label,
              e_attr: attr,
              is_src_curr_pat: true,
            })
            .await;

            let is_matched_data_es_empty = matched_data_es.is_empty();

            // group by: next data_vertex
            for e in matched_data_es {
              next_vid_grouped_conn_pat_strs
                .entry(e.dst_vid().to_owned())
                .or_insert_with(Vec::new)
                .push(pat_e.eid().to_owned());
              next_vid_grouped_conn_es
                .entry(e.dst_vid().to_owned())
                .or_insert_with(Vec::new)
                .push(e);
            }
            is_matched_data_es_empty
          } else {
            next_pat_vid = pat_e.src_vid();

            let matched_data_es = incremental_match(LoadWithCondCtx {
              pattern_vs,
              storage_adapter,
              curr_matched_dg: matched_dg,
              frontier_vid,
              curr_pat_e: pat_e,
              e_label: label,
              e_attr: attr,
              is_src_curr_pat: false,
            })
            .await;

            let is_matched_data_es_empty = matched_data_es.is_empty();

            // group by: next data_vertex
            for e in matched_data_es {
              next_vid_grouped_conn_pat_strs
                .entry(e.src_vid().to_owned())
                .or_insert_with(Vec::new)
                .push(pat_e.eid().to_owned());
              next_vid_grouped_conn_es
                .entry(e.src_vid().to_owned())
                .or_insert_with(Vec::new)
                .push(e);
            }
            is_matched_data_es_empty
          };

          if is_matched_data_es_empty {
            continue;
          }

          is_frontier_connected = true;

          // build `expanding_graph`
          // note that each `next_data_vertex` holds a `expanding_graph`
          for (key, edges) in next_vid_grouped_conn_es {
            let mut expanding_graph = ExpandGraph::from(matched_dg);
            let pat_strs = next_vid_grouped_conn_pat_strs
              .remove(&key)
              .unwrap_or_default();

            expanding_graph
              .update_valid_dangling_edges(edges.iter().zip(pat_strs.iter().map(String::as_str)))
              .await;
            self
              .next_pat_grouped_expanding
              .entry(next_pat_vid.to_owned())
              .or_default()
              .push(expanding_graph);
          }
        }

        if is_frontier_connected {
          connected_data_vids.insert(frontier_vid.to_owned());
        }
      }
    }

    self.all_matched.clear();

    connected_data_vids
  }
}

struct LoadWithCondCtx<'a, S: StorageAdapter> {
  pattern_vs: &'a HashMap<String, PatternVertex>,
  storage_adapter: &'a S,
  curr_matched_dg: &'a DynGraph,
  frontier_vid: VidRef<'a>,
  curr_pat_e: &'a PatternEdge,
  e_label: LabelRef<'a>,
  e_attr: Option<&'a PatternAttr>,
  is_src_curr_pat: bool,
}

async fn incremental_match<'a, S: StorageAdapter>(ctx: LoadWithCondCtx<'a, S>) -> Vec<DataEdge> {
  let next_pat_vid = if ctx.is_src_curr_pat {
    ctx.curr_pat_e.dst_vid()
  } else {
    ctx.curr_pat_e.src_vid()
  };

  // load all edges first
  let potential_edges = if ctx.is_src_curr_pat {
    ctx
      .storage_adapter
      .load_e_with_src(ctx.frontier_vid, ctx.e_label, ctx.e_attr)
      .await
  } else {
    ctx
      .storage_adapter
      .load_e_with_dst(ctx.frontier_vid, ctx.e_label, ctx.e_attr)
      .await
  };

  // build future condition
  let edge_futures = potential_edges
    .into_iter()
    .filter(|e| !ctx.curr_matched_dg.has_eid(e.eid()))
    .map(|e| async {
      let satisfies = does_data_v_satisfy_pattern(
        if ctx.is_src_curr_pat {
          e.dst_vid()
        } else {
          e.src_vid()
        },
        next_pat_vid,
        ctx.pattern_vs,
        ctx.storage_adapter,
      )
      .await;
      if satisfies { Some(e) } else { None }
    });

  // exec
  let results = future::join_all(edge_futures).await;

  results.into_iter().flatten().collect::<Vec<_>>()
}

impl CBucket {
  pub async fn build_from_a_group(
    a_group: impl IntoIterator<Item = ExpandGraph>,
    loaded_v_pat_pairs: impl IntoIterator<Item = (DataVertex, String)>,
  ) -> Self {
    let loaded = loaded_v_pat_pairs.into_iter().collect::<Vec<_>>();

    let mut all_expanded = vec![];
    let mut expanded_with_frontiers = HashMap::new();

    let a_group = a_group.into_iter().collect::<Vec<_>>();

    for (idx, mut expanding) in a_group.into_iter().enumerate() {
      let valid_targets = expanding
        .update_valid_target_vertices(loaded.iter().map(|(v, p)| (v, p.as_str())))
        .await;

      all_expanded.push(expanding);
      expanded_with_frontiers
        .entry(idx)
        .or_insert_with(Vec::new)
        .extend(valid_targets);
    }

    Self {
      all_expanded,
      expanded_with_frontiers,
    }
  }

  pub async fn build_from_t(
    t_bucket: &mut TBucket,
    loaded_v_pat_pairs: impl IntoIterator<Item = (DataVertex, String)>,
  ) -> Self {
    let loaded = loaded_v_pat_pairs.into_iter().collect::<Vec<_>>();
    let mut all_expanded = vec![];
    let mut expanded_with_frontiers = HashMap::new();

    for (idx, expanding) in t_bucket.expanding_graphs.iter_mut().enumerate() {
      let valid_targets = expanding
        .update_valid_target_vertices(loaded.iter().map(|(v, p)| (v, p.as_str())))
        .await;

      all_expanded.push(expanding.to_owned());
      expanded_with_frontiers
        .entry(idx)
        .or_insert_with(Vec::new)
        .extend(valid_targets);
    }

    Self {
      all_expanded,
      expanded_with_frontiers,
    }
  }
}

impl TBucket {
  pub async fn build_from_a_a(
    left: impl IntoIterator<Item = ExpandGraph>,
    right: impl IntoIterator<Item = ExpandGraph>,
    target_pat_vid: VidRef<'_>,
  ) -> Self {
    let left_group = left.into_iter().collect();
    let right_group = right.into_iter().collect();

    let expanding_graphs = Self::expand_edges_of_two(left_group, right_group).await;
    Self {
      target_pat_vid: target_pat_vid.to_owned(),
      expanding_graphs,
    }
  }

  pub async fn build_from_t_a(
    t_bucket: TBucket,
    a_group: impl IntoIterator<Item = ExpandGraph>,
  ) -> Self {
    let left_group = t_bucket.expanding_graphs;
    let right_group = a_group.into_iter().collect();

    let expanding_graphs = Self::expand_edges_of_two(left_group, right_group).await;
    Self {
      target_pat_vid: t_bucket.target_pat_vid,
      expanding_graphs,
    }
  }

  async fn expand_edges_of_two(
    left_group: Vec<ExpandGraph>,
    right_group: Vec<ExpandGraph>,
  ) -> Vec<ExpandGraph> {
    let mut futures = vec![];

    for outer in left_group.iter() {
      for inner in right_group.iter() {
        let future = union_then_intersect_on_connective_v(outer, inner);
        futures.push(future);
      }
    }

    let results = future::join_all(futures).await;
    results.into_iter().flatten().collect::<Vec<_>>()
  }
}
