use super::{JsonValue, Req, Filter};
use hyper::server as http;
use hyper::Error as HyperError;
use hyper::header::{ContentLength, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper;
use hyper::Chunk as HyperChunk;
use futures::{Poll, Stream, Async, Future};
use futures::stream::BoxStream;
use futures::future::BoxFuture;
use futures::sync::mpsc;
use Sender;
use std::sync::Arc;
use futures;

type ChunkReceiver = BoxStream<HyperChunk, ()>;

pub struct Reply {
  data: ReplyData,
  code: i64, // TODO replace with enum of errors, etc
  req: Option<Req>,
}

enum ReplyData {
  Value(JsonValue),
  Stream(ChunkReceiver),
}

impl Reply {
  // TODO refine this? currently only really should be used internally.
  pub fn new(code: i64, req: Option<Req>, data: JsonValue) -> Reply {
    Reply {
      code: code,
      req: req,
      data: ReplyData::Value(data),
    }
  }

  pub fn to_http(self) -> http::Response<Body> {
    let resp = http::Response::new();

    match self.data {
      ReplyData::Value(val) => {
        let resp_str = val.to_string();
        resp
          .with_header(ContentLength(resp_str.len() as u64))
          .with_body(Body::Once(Some(resp_str.into())))
      },
      ReplyData::Stream(stream) => {
        resp
          .with_header(ContentType(Mime(TopLevel::Text, SubLevel::EventStream, vec![(hyper::mime::Attr::Charset, hyper::mime::Value::Utf8)])))
          .with_body(Body::Stream(stream))
      },
    }
  }

  pub fn new_streamed(code: i64, req: Option<Req>, filter: Arc<Box<Filter>>) -> (Sender, Reply) {
    let (tx, rx) = mpsc::unbounded();
    let rx = rx
      .and_then(move |item: JsonValue| {
        let rep = Reply {
          code: 200,
          req: None, // TODO
          data: ReplyData::Value(item),
        };
        filter.handle(rep).then(|filter_res| -> Result<Option<JsonValue>, ()> {
          match filter_res {
            Ok(rep) => match rep.data {
              ReplyData::Value(dat) => Ok(Some(dat)),
              _ => Ok(None),
            },
            Err(_) => Ok(None),
          }
        })
      })
      .filter_map(|item: Option<JsonValue>| -> Option<JsonValue> {
        item
      })
      .map(|val| -> HyperChunk {
        format!("data:{}\n\n", val).into()
      })
      .boxed();
    let reply = Reply {
      code: code,
      req: req,
      data: ReplyData::Stream(rx)
    };
    let sender = Sender::new(tx);
    (sender, reply)
  }
}

/// A `Stream` for `HyperChunk`s used in requests and responses.
pub enum Body {
  Once(Option<HyperChunk>),
  Stream(ChunkReceiver),
}

impl Stream for Body {
  type Item = HyperChunk;
  type Error = HyperError;

  fn poll(&mut self) -> Poll<Option<HyperChunk>, HyperError> {
    match self {
      &mut Body::Once(ref mut opt) => Ok(Async::Ready(opt.take())),
      &mut Body::Stream(ref mut stream) => {
        match stream.poll() {
          Ok(u) => Ok(u),
          Err(()) => Err(HyperError::Incomplete) // TODO FIX THIS ERROR
        }
      }
    }
  }
}
