use crate::{
  matching_ctx::MatchingCtx,
  schemas::{DataEdge, DataVertex, Instruction},
  storage::StorageAdapter,
  utils::dyn_graph::DynGraph,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct InitOperator<S: StorageAdapter> {
  pub(crate) storage_adapter: Arc<Mutex<S>>,
  pub(crate) ctx: Arc<Mutex<MatchingCtx>>,
}

impl<S: StorageAdapter> InitOperator<S> {
  pub async fn execute(&mut self, instr: &Instruction) {
    let instr_json = serde_json::to_string_pretty(instr).unwrap();
    println!("{instr_json}\n");

    let mut ctx = self.ctx.lock().await;

    let pattern_v = ctx.get_pattern_v(&instr.vid).cloned();
    if pattern_v.is_none() {
      println!(
        "No pattern_v found for '{}'\n",
        instr.single_op.as_ref().unwrap()
      );
      return;
    }

    let pattern_v = pattern_v.unwrap();

    ctx.init_f_block(&instr.target_var);

    let label = pattern_v.label.as_str();
    let attr = pattern_v.attr.as_ref();
    let matched_vs = self.storage_adapter.lock().await.load_v(label, attr).await;

    let matched_results = matched_vs
      .into_iter()
      .filter(|data_v| !ctx.expanded_data_vids.contains(&data_v.vid))
      .map(|data_v| {
        let mut matched_dg = DynGraph::<DataVertex, DataEdge>::default();
        let frontier_vid = data_v.vid.clone();
        matched_dg.update_v(data_v, pattern_v.vid.clone());
        (matched_dg, frontier_vid)
      })
      .collect::<Vec<_>>();

    for (matched_dg, frontier_vid) in matched_results {
      ctx.append_to_f_block(&instr.target_var, matched_dg, &frontier_vid);
    }
  }
}
