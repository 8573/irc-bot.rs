use super::Error;
use futures::Async;
use futures::AsyncSink;
use futures::Poll;
use futures::Sink;
use futures::StartSend;
use pircolate;

pub struct NullSink;

impl Sink for NullSink {
    type SinkItem = pircolate::Message;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        error!("Connection not yet ready. Discarding outgoing message: {:?}",
               item);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}
