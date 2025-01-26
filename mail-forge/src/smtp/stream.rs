use std::pin::Pin;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader};
use tokio_rustls::server::TlsStream;

pub enum StreamType<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    Plain(BufReader<S>),
    Tls(BufReader<TlsStream<S>>),
}

impl<S> AsyncBufRead for StreamType<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        match self.get_mut() {
            StreamType::Plain(inner) => Pin::new(inner).poll_fill_buf(cx),
            StreamType::Tls(inner) => Pin::new(inner).poll_fill_buf(cx),
        }
    }

    fn consume(self: std::pin::Pin<&mut Self>, amt: usize) {
        match self.get_mut() {
            StreamType::Plain(inner) => Pin::new(inner).consume(amt),
            StreamType::Tls(inner) => Pin::new(inner).consume(amt),
        }
    }
}

impl<S> AsyncWrite for StreamType<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            StreamType::Plain(inner) => Pin::new(inner).poll_write(cx, buf),
            StreamType::Tls(inner) => Pin::new(inner).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            StreamType::Plain(inner) => Pin::new(inner).poll_flush(cx),
            StreamType::Tls(inner) => Pin::new(inner).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            StreamType::Plain(inner) => Pin::new(inner).poll_shutdown(cx),
            StreamType::Tls(inner) => Pin::new(inner).poll_shutdown(cx),
        }
    }
}

impl<S> AsyncRead for StreamType<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            StreamType::Plain(inner) => Pin::new(inner).poll_read(cx, buf),
            StreamType::Tls(inner) => Pin::new(inner).poll_read(cx, buf),
        }
    }
}
