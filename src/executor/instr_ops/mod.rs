use crate::{
  matching_ctx::MatchingCtx,
  schemas::{Instruction, InstructionType::*, STR_TUPLE_SPLITTER, VarPrefix},
  storage::AdvancedStorageAdapter,
};
use foreach::ForeachOperator;
use get_adj::GetAdjOperator;
use init::InitOperator;
use intersect::IntersectOperator;
use itertools::Itertools;
use report::ReportOperator;
use std::{str::FromStr, sync::Arc};

pub mod foreach;
pub mod get_adj;
pub mod init;
pub mod intersect;
pub mod report;

#[inline]
pub(crate) fn resolve_var(target_var: &str) -> (VarPrefix, &str) {
  let splitted = target_var.split(STR_TUPLE_SPLITTER).collect_vec();
  let var_type = splitted[0];
  let var_name = splitted[1];
  (VarPrefix::from_str(var_type).unwrap(), var_name)
}

pub enum InstrOperator<S: AdvancedStorageAdapter + 'static> {
  Init(InitOperator<S>),
  GetAdj(GetAdjOperator<S>),
  Foreach(ForeachOperator),
  Intersect(IntersectOperator<S>),
  Report(ReportOperator),
}

impl<S: AdvancedStorageAdapter + 'static> InstrOperator<S> {
  pub async fn execute(&mut self, instr: &Instruction) {
    match self {
      InstrOperator::Init(operator) => operator.execute(instr).await,
      InstrOperator::GetAdj(operator) => operator.execute(instr).await,
      InstrOperator::Foreach(operator) => operator.execute(instr).await,
      InstrOperator::Intersect(operator) => operator.execute(instr).await,
      InstrOperator::Report(operator) => operator.execute(instr).await,
    };
  }
}

pub struct InstrOperatorFactory;

impl InstrOperatorFactory {
  pub fn create<S: AdvancedStorageAdapter>(
    instr: &Instruction,
    storage_adapter: Arc<S>,
    ctx: Arc<MatchingCtx>,
  ) -> InstrOperator<S> {
    match instr.type_ {
      Init => InstrOperator::Init(InitOperator {
        storage_adapter,
        ctx,
      }),
      GetAdj => InstrOperator::GetAdj(GetAdjOperator {
        storage_adapter,
        ctx,
      }),
      Foreach => InstrOperator::Foreach(ForeachOperator { ctx }),
      Intersect => InstrOperator::Intersect(IntersectOperator {
        storage_adapter,
        ctx,
      }),
      Report => InstrOperator::Report(ReportOperator { ctx }),
      TCache => unimplemented!("`TCache` operator is not implemented yet."),
    }
  }
}
