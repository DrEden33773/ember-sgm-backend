use super::*;

impl FBucket {
  pub async fn from_c_bucket(c_bucket: CBucket) -> Self {
    let mut all_matched = vec![];
    let mut matched_with_frontiers = HashMap::new();

    let all_expanded = c_bucket.all_expanded;
    let mut expanded_with_frontiers = c_bucket.expanded_with_frontiers;

    for (idx, matched) in all_expanded.into_iter().enumerate() {
      all_matched.push(matched.into());
      matched_with_frontiers
        .entry(idx)
        .or_insert_with(Vec::new)
        .extend(expanded_with_frontiers.remove(&idx).unwrap_or_default());
    }

    Self {
      all_matched,
      matched_with_frontiers,
    }
  }
}
