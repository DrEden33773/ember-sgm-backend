use tokio::time::Instant;

pub mod apriori;
pub mod dyn_graph;
pub mod expand_graph;
pub mod parallel;
pub mod pretty_dump;
pub mod simd_utils;

pub async fn time_async<F: Future<Output = O>, O>(future: F) -> (O, f64) {
  let start = Instant::now();
  let result = future.await;
  let elapsed = start.elapsed().as_millis_f64();
  (result, elapsed)
}

#[cfg(not(feature = "enable_log"))]
pub async fn time_async_with_desc<F: Future<Output = O>, O>(future: F, _desc: String) -> O {
  let (result, _elapsed) = time_async(future).await;
  // println!("{} ✅  {_elapsed:.2}ms\n", _desc);
  result
}

#[cfg(feature = "enable_log")]
#[tracing::instrument(skip(future, desc))]
pub async fn time_async_with_desc<F: Future<Output = O>, O>(future: F, desc: String) -> O {
  let (result, elapsed) = time_async(future).await;
  tracing::info!("{} ✅  {elapsed:.2}ms\n", desc);
  result
}
