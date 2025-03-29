use crate::{
  matching_ctx::MatchingCtx, schemas::*, storage::StorageAdapter, utils::dyn_graph::DynGraph,
};
use hashbrown::HashMap;
use instr_ops::InstrOperatorFactory;
use itertools::Itertools;
use std::{collections::VecDeque, sync::Arc};
use tokio::sync::Mutex;

pub mod instr_ops;

#[derive(Clone)]
pub struct ExecEngine<S: StorageAdapter> {
  pub(crate) plan_data: PlanData,
  pub(crate) matching_ctx: Arc<Mutex<MatchingCtx>>,
  pub(crate) storage_adapter: Arc<Mutex<S>>,
}

impl<S: StorageAdapter> ExecEngine<S> {
  pub async fn build_from_json(plan_json_content: &str) -> Self {
    let plan_data: PlanData = serde_json::from_str(plan_json_content).unwrap();
    let matching_ctx = Arc::new(Mutex::new(MatchingCtx::new(&plan_data)));
    let storage_adapter = Arc::new(Mutex::new(S::async_default().await));
    Self {
      plan_data,
      matching_ctx,
      storage_adapter,
    }
  }

  pub async fn exec_without_final_merge(&mut self) -> Vec<Vec<DynGraph>> {
    let mut operators = Vec::with_capacity(self.plan_data.instructions.len());
    for instr in self.plan_data.instructions.iter() {
      let operator = InstrOperatorFactory::create(
        instr,
        self.storage_adapter.clone(),
        self.matching_ctx.clone(),
      );
      operators.push(operator);
    }

    for (operator, instr) in operators.iter_mut().zip(self.plan_data.instructions.iter()) {
      operator.execute(instr).await;
    }

    self
      .matching_ctx
      .lock()
      .await
      .grouped_partial_matches
      .drain(0..)
      .collect()
  }

  pub async fn exec(&mut self) -> Vec<DynGraph> {
    let unmerged_results = self
      .exec_without_final_merge()
      .await
      .into_iter()
      .filter(|v| !v.is_empty())
      .collect::<Vec<_>>();

    if unmerged_results.is_empty() {
      return vec![];
    }

    let mut result = vec![];

    let plan_v_pat_cnt = self
      .plan_data
      .pattern_vs()
      .keys()
      .map(|v_pat| (v_pat.to_owned(), 1))
      .collect::<HashMap<_, usize>>();
    let plan_e_pat_cnt = self
      .plan_data
      .pattern_es()
      .keys()
      .map(|e_pat| (e_pat.to_owned(), 1))
      .collect::<HashMap<_, usize>>();

    let could_match_the_whole_pattern = |graph: &DynGraph| -> bool {
      let mut graph_v_pat_cnt = HashMap::new();
      let mut graph_e_pat_cnt = HashMap::new();

      for pat in graph.v_patterns.values().cloned() {
        let cnt = graph_v_pat_cnt.entry(pat).or_insert(0);
        *cnt += 1;
      }
      for pat in graph.e_patterns.values().cloned() {
        let cnt = graph_e_pat_cnt.entry(pat).or_insert(0);
        *cnt += 1;
      }

      graph_v_pat_cnt == plan_v_pat_cnt && graph_e_pat_cnt == plan_e_pat_cnt
    };

    for mut combination in unmerged_results.into_iter().multi_cartesian_product() {
      let mut successors = combination.drain(1..).collect::<VecDeque<_>>();
      let mut curr = combination.pop().unwrap();
      while let Some(next) = successors.pop_front() {
        let new = curr | next;
        curr = new;
      }
      result.push(curr);
    }

    result
      .into_iter()
      .filter(&could_match_the_whole_pattern)
      .collect::<Vec<_>>()
  }
}
