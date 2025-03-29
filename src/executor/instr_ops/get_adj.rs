use super::resolve_var;
use crate::{
  matching_ctx::{MatchingCtx, buckets::ABucket},
  schemas::Instruction,
  storage::StorageAdapter,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct GetAdjOperator<S: StorageAdapter> {
  pub(crate) storage_adapter: Arc<Mutex<S>>,
  pub(crate) ctx: Arc<Mutex<MatchingCtx>>,
}

impl<S: StorageAdapter> GetAdjOperator<S> {
  pub async fn execute(&mut self, instr: &Instruction) {
    println!("{instr:#?}\n");

    // to resolve current `pattern_vid`
    let (_, curr_pat_vid) = resolve_var(instr.single_op.as_ref().unwrap());

    let mut ctx = self.ctx.lock().await;

    let f_bucket = ctx.pop_from_f_block(instr.single_op.as_ref().unwrap());
    let mut a_bucket = ABucket::from_f_bucket(f_bucket, curr_pat_vid);

    // must init a_block first
    ctx.init_a_block(&instr.target_var);

    let connected_data_vids = {
      let pattern_vs = ctx.pattern_vs();
      let pattern_es = ctx.fetch_pattern_e_batch(instr.expand_eids.iter().map(String::as_str));
      let storage_adapter = self.storage_adapter.lock().await;

      // core logic: incremental load new edges
      a_bucket
        .incremental_load_new_edges(pattern_es, pattern_vs, &*storage_adapter)
        .await
    };

    // update the `block` and `extended data vid set`
    ctx.update_a_block(&instr.target_var, a_bucket);
    ctx.update_extended_data_vids(connected_data_vids);
  }
}
