extern crate backtalk;
use backtalk::*;
extern crate futures;
use futures::Future;
#[macro_use]
extern crate serde_json;

fn main() {
  let mut server = Server::new();
  // code will be added here
  server.listen("127.0.0.1:3000");
}