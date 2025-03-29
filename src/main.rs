use dotenv::dotenv;
use ember_sgm_backend::demos::complex_sf01::*;
use tokio::io;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> io::Result<()> {
  dotenv().ok();

  ic_12_on_sf_01().await
}
