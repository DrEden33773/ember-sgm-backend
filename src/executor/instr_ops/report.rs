use crate::{matching_ctx::MatchingCtx, schemas::Instruction, utils::dyn_graph::DynGraph};
use hashbrown::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ReportOperator {
  pub(crate) ctx: Arc<Mutex<MatchingCtx>>,
}

impl ReportOperator {
  pub async fn execute(&mut self, instr: &Instruction) {
    println!("{instr:#?}\n");

    let mut ctx = self.ctx.lock().await;

    let plan_v_pat_cnt = ctx
      .plan_data
      .pattern_vs()
      .keys()
      .map(|v_pat| (v_pat.to_owned(), 1))
      .collect::<HashMap<_, usize>>();
    let plan_e_pat_cnt = ctx
      .plan_data
      .pattern_es()
      .keys()
      .map(|e_pat| (e_pat.to_owned(), 1))
      .collect::<HashMap<_, usize>>();

    let could_match_partial_pattern = |graph: &DynGraph| -> bool {
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

      for (v_pat, cnt) in graph_v_pat_cnt {
        if let Some(&plan_cnt) = plan_v_pat_cnt.get(&v_pat) {
          if cnt > plan_cnt {
            return false;
          }
        } else {
          return false;
        }
      }
      for (e_pat, cnt) in graph_e_pat_cnt {
        if let Some(&plan_cnt) = plan_e_pat_cnt.get(&e_pat) {
          if cnt > plan_cnt {
            return false;
          }
        } else {
          return false;
        }
      }

      true
    };

    let f_buckets: Vec<_> = ctx.f_block.drain().collect();

    let mut filtered_groups = Vec::new();
    for (_, f_bucket) in f_buckets {
      let curr_group = f_bucket.all_matched.into_iter().collect::<Vec<_>>();

      let filtered_group = curr_group
        .into_iter()
        .filter(&could_match_partial_pattern)
        .collect::<Vec<_>>();

      filtered_groups.push(filtered_group);
    }

    for curr_group in filtered_groups {
      ctx.grouped_partial_matches.push(curr_group);
    }
  }
}
