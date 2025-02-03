use bsp::{Bsp, LUMP_COUNT};

fn main() {
  let Some(path) = std::env::args().nth(1) else {
    eprintln!("Usage: ./{} <file>", env!("CARGO_BIN_NAME"));
    std::process::exit(1);
  };

  println!("reading path: {path}");
  let mut contents = std::fs::read(path).expect("failed to open file");

  let bsp = Bsp::new(&mut contents).expect("failed to deserialize bsp");
  println!("{bsp:#?}");

  for i in 0..LUMP_COUNT {
    let lump = bsp.lump(i);
    println!("lump {i}: {} bytes", lump.len());
  }
}
