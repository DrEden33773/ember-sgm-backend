use std::env;

use super::{AsyncDefault, StorageAdapter};
use crate::schemas::*;
use neo4rs::*;

#[derive(Clone)]
pub struct Neo4jStorageAdapter {
  pub(crate) graph: Graph,
}

impl AsyncDefault for Neo4jStorageAdapter {
  async fn async_default() -> Self {
    let uri = env::var("NEO4J_URI").unwrap();
    let username = env::var("NEO4J_USERNAME").unwrap();
    let password = env::var("NEO4J_PASSWORD").unwrap();
    let db_name = env::var("NEO4J_DATABASE").unwrap();
    let config = ConfigBuilder::default()
      .uri(uri)
      .user(username)
      .password(password)
      .db(db_name)
      .fetch_size(1000)
      .max_connections(num_cpus::get())
      .build()
      .unwrap();

    println!("Connecting to Neo4j database...\n");

    let graph = Graph::connect(config).await.unwrap();

    println!("Connected to Neo4j database.\n");

    Self { graph }
  }
}

impl From<(Row, LabelRef<'_>)> for DataEdge {
  fn from((row, e_label): (Row, LabelRef)) -> Self {
    let eid = row.get("eid").unwrap();
    let src_vid = row.get("src_vid").unwrap();
    let dst_vid = row.get("dst_vid").unwrap();
    let label = e_label.to_string();
    let attrs = row.get("props").unwrap();

    DataEdge {
      eid,
      src_vid,
      dst_vid,
      label,
      attrs,
    }
  }
}

impl StorageAdapter for Neo4jStorageAdapter {
  async fn get_v(&self, vid: VidRef<'_>) -> Option<DataVertex> {
    let mut query_str = "MATCH (v)\n".to_string();
    query_str += &format!("WHERE elementId(v) = '{vid}'\n");
    query_str += "
      RETURN
        properties(v) as props,
        labels(v) as v_label
    ";
    let query_str = query_str.split_whitespace().collect::<Vec<_>>().join(" ");
    // println!(">>> {query_str}\n");

    let mut result = self.graph.execute(query(&query_str)).await.unwrap();
    let row = result.next().await.unwrap()?;

    let vid = vid.to_string();
    let labels: Vec<String> = row.get("v_label").unwrap();
    let label = labels[0].to_owned();
    let attrs = row.get("props").unwrap();

    Some(DataVertex { vid, label, attrs })
  }

  async fn load_v(&self, v_label: LabelRef<'_>, v_attr: Option<&PatternAttr>) -> Vec<DataVertex> {
    let mut query_str = format!("MATCH (v:{v_label})\n");
    if let Some(attr) = v_attr {
      let constraint = attr.to_neo4j_constraint("v");
      query_str += &format!("WHERE {constraint}\n");
    }
    query_str += "
      RETURN
        properties(v) as props,
        elementId(v) as vid
    ";
    let query_str = query_str.split_whitespace().collect::<Vec<_>>().join(" ");
    // println!(">>> {query_str}\n");

    let mut result = self.graph.execute(query(&query_str)).await.unwrap();
    let mut ret = vec![];

    while let Some(row) = result.next().await.unwrap() {
      let vid = row.get("vid").unwrap();
      let label = v_label.to_string();
      let attrs = row.get("props").unwrap();

      ret.push(DataVertex { vid, label, attrs });
    }

    ret
  }

  async fn load_e(&self, e_label: LabelRef<'_>, e_attr: Option<&PatternAttr>) -> Vec<DataEdge> {
    let mut query_str = format!("MATCH (src)-[e:{e_label}]->(dst)\n");
    if let Some(attr) = e_attr {
      let constraint = attr.to_neo4j_constraint("e");
      query_str += &format!("WHERE {constraint}\n");
    }
    query_str += "
      RETURN
        elementId(e) AS eid,
        properties(e) AS props,
        elementId(src) AS src_vid,
        elementId(dst) AS dst_vid
    ";
    let query_str = query_str.split_whitespace().collect::<Vec<_>>().join(" ");
    // println!(">>> {query_str}\n");

    let mut result = self.graph.execute(query(&query_str)).await.unwrap();
    let mut ret = vec![];

    while let Some(row) = result.next().await.unwrap() {
      ret.push(DataEdge::from((row, e_label)));
    }
    ret
  }

  async fn load_e_with_src(
    &self,
    src_vid: VidRef<'_>,
    e_label: LabelRef<'_>,
    e_attr: Option<&PatternAttr>,
  ) -> Vec<DataEdge> {
    let mut query_str = format!("MATCH (src)-[e:{e_label}]->(dst)\n");
    query_str += &format!("WHERE elementId(src) = '{src_vid}'\n");
    if let Some(attr) = e_attr {
      let constraint = attr.to_neo4j_constraint("e");
      query_str += &format!("AND {constraint}\n");
    }
    query_str += "
      RETURN
        elementId(e) AS eid,
        properties(e) AS props,
        elementId(src) AS src_vid,
        elementId(dst) AS dst_vid
    ";
    let query_str = query_str.split_whitespace().collect::<Vec<_>>().join(" ");
    // println!(">>> {query_str}\n");

    let mut result = self.graph.execute(query(&query_str)).await.unwrap();
    let mut ret = vec![];

    while let Some(row) = result.next().await.unwrap() {
      ret.push(DataEdge::from((row, e_label)));
    }
    ret
  }

  async fn load_e_with_dst(
    &self,
    dst_vid: VidRef<'_>,
    e_label: LabelRef<'_>,
    e_attr: Option<&PatternAttr>,
  ) -> Vec<DataEdge> {
    let mut query_str = format!("MATCH (src)-[e:{e_label}]->(dst)\n");
    query_str += &format!("WHERE elementId(dst) = '{dst_vid}'\n");
    if let Some(attr) = e_attr {
      let constraint = attr.to_neo4j_constraint("e");
      query_str += &format!("AND {constraint}\n");
    }
    query_str += "
      RETURN
        elementId(e) AS eid,
        properties(e) AS props,
        elementId(src) AS src_vid,
        elementId(dst) AS dst_vid
    ";
    let query_str = query_str.split_whitespace().collect::<Vec<_>>().join(" ");
    // println!(">>> {query_str}\n");

    let mut result = self.graph.execute(query(&query_str)).await.unwrap();
    let mut ret = vec![];

    while let Some(row) = result.next().await.unwrap() {
      ret.push(DataEdge::from((row, e_label)));
    }
    ret
  }
}
