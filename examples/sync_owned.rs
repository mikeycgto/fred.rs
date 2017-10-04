#![allow(unused_variables)]
#![allow(unused_imports)]

extern crate redis_client;
extern crate tokio_core;
extern crate tokio_timer;
extern crate futures;

use redis_client::RedisClient;
use redis_client::sync::owned::RedisClientRemote;
use redis_client::types::*;
use redis_client::error::*;

use tokio_core::reactor::Core;
use futures::Future;
use futures::stream::{
  self,
  Stream
};
use futures::future::{
  self,
  Either
};

use tokio_timer::Timer;
use std::time::Duration;
use std::thread;

const KEY: &'static str = "foo";

fn main() {
  let sync_client = RedisClientRemote::new();

  let t_sync_client = sync_client.clone();
  // create an event loop in a separate thread and run the client there
  let event_loop_jh = thread::spawn(move || {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let config = RedisConfig::default();
    let client = RedisClient::new(config);

    // future that runs the underlying connection
    let connection = client.connect(&handle);

    // future that runs the remote interface
    let remote = t_sync_client.init(client);

    let composed = connection.join(remote);
    let _ = core.run(composed).unwrap();
  });

  // block this thread until the underlying client is connected
  let _ = sync_client.on_connect().wait();

  // create two threads, one to increment a value every second, and a second to read the same value every second
  let reader_client = sync_client.clone();
  let reader_jh = thread::spawn(move || {
    let timer = Timer::default();
    let dur = Duration::from_millis(1000);

    // read the value every second
    let timer_ft = timer.interval(dur).from_err::<RedisError>().fold(reader_client, |reader_client, _| {

      reader_client.get(KEY).and_then(|(client, value)| {
        if let Some(value) = value {
          println!("Reader read key {:?} with value {:?}", KEY, value);
        }

        Ok(client)
      })
    });

    // block this thread to run the timer
    let _ = timer_ft.wait();
  });

  let writer_client = sync_client.clone();
  let writer_jh = thread::spawn(move || {
    let timer = Timer::default();
    let dur = Duration::from_millis(1000);

    // increment the value every second
    let timer_ft = timer.interval(dur).from_err::<RedisError>().fold(writer_client, |writer_client, _| {

      writer_client.incr(KEY).and_then(|(client, value)| {
        println!("Writer incremented key {:?} to {:?}", KEY, value);

        Ok(client)
      })
    });

    // block this thread to run the timer
    let _ = timer_ft.wait();
  });

  // block this thread while the three child threads run
  let _ = event_loop_jh.join();
  let _ = reader_jh.join();
  let _ = writer_jh.join();
}