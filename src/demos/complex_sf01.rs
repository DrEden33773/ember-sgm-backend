use crate::{executor::ExecEngine, storage::*};
use project_root::get_project_root;
use tokio::{fs, io};

async fn exec(plan_name: &str) -> io::Result<()> {
  let mut path = get_project_root()?;
  path.push("resources");
  path.push("plan");
  path.push(plan_name);
  let plan_json_content = fs::read_to_string(path).await?;

  let result = ExecEngine::<Neo4jStorageAdapter>::build_from_json(&plan_json_content)
    .await
    .exec()
    .await;

  println!("{:#?}\n", &result);

  println!("COUNT(result) = {}\n", result.len());
  Ok(())
}

pub async fn ic_1_on_sf_01() -> io::Result<()> {
  println!("\nQuerying 'IC-1' on 'SF0.1' ...\n");
  exec("ldbc-ic-1-single-directed-knows.json").await
}

pub async fn ic_11_on_sf_01() -> io::Result<()> {
  println!("\nQuerying 'IC-11' on 'SF0.1' ...\n");
  exec("ldbc-ic-11-single-directed-knows.json").await
}
