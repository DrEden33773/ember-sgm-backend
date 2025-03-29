use crate::{
  matching_ctx::MatchingCtx,
  schemas::{DataEdge, DataVertex, Instruction},
  storage::StorageAdapter,
  utils::dyn_graph::DynGraph,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct InitOperator<S: StorageAdapter> {
  pub(crate) storage_adapter: Arc<Mutex<S>>,
  pub(crate) ctx: Arc<Mutex<MatchingCtx>>,
}

impl<S: StorageAdapter> InitOperator<S> {
  pub async fn execute(&mut self, instr: &Instruction) {
    println!("{instr:#?}\n");

    let pattern_v = { self.ctx.lock().await }
      .get_pattern_v(&instr.vid)
      .to_owned();

    let label = pattern_v.label.as_str();
    let attr = pattern_v.attr.as_ref();

    // load vertices
    let matched_vs = { self.storage_adapter.lock().await }
      .load_v(label, attr)
      .await;

    // must init f_block first
    let mut ctx = self.ctx.lock().await;
    ctx.init_f_block(&instr.target_var);

    // filter-out if the vertex has already been expanded
    let unexpanded_matched_vs = matched_vs
      .into_par_iter()
      .filter(|data_v| !ctx.expanded_data_vids.contains(&data_v.vid))
      .collect_vec_list();

    // prepare for: updating the block
    let pre = unexpanded_matched_vs
      .into_par_iter()
      .flatten()
      .map(|data_v| {
        let frontier_vid = data_v.vid.to_owned();
        let mut matched_dg = DynGraph::<DataVertex, DataEdge>::default();
        matched_dg.update_v(data_v, pattern_v.vid.clone());

        (&instr.target_var, matched_dg, frontier_vid)
      })
      .collect_vec_list();

    // update f_block
    for (target_var, matched_dg, frontier_vid) in pre.into_iter().flatten() {
      ctx.append_to_f_block(target_var, matched_dg, &frontier_vid);
    }
  }
}
