#[cfg(all(feature = "jsonrpsee", feature = "jsonrpc"))]
compile_error!("`jsonrpc` and `jsonrpsee` features are mutually exclusive, please enable only one");

use prometheus::TextEncoder;

pub use lazy_static::lazy_static;
pub use prometheus::{
    exponential_buckets, histogram_opts, labels, linear_buckets, opts, register_histogram, register_histogram_vec,
    register_int_counter, register_int_counter_vec, register_int_gauge, register_int_gauge_vec, Error, Histogram,
    HistogramTimer, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec,
};

#[cfg(feature = "jsonrpc")]
pub use jsonrpc_core::{Error as JsonRpcError, ErrorCode as JsonRpcErrorCode};
#[cfg(feature = "jsonrpsee")]
pub use jsonrpsee::{core::Error as JsonRpcError, types::error::CallError as JsonRpcCallError};

#[cfg(feature = "derive")]
pub use prometheus_fire_derive::Metrics;

#[cfg(feature = "jsonrpsee")]
#[jsonrpsee::proc_macros::rpc(server)]
pub trait MetricsRpc {
    #[method(name = "metrics")]
    fn metrics(&self) -> jsonrpsee::core::RpcResult<String>;
}

#[cfg(feature = "jsonrpc")]
#[jsonrpc_derive::rpc(server)]
pub trait MetricsRpc {
    #[rpc(name = "metrics")]
    fn metrics(&self) -> jsonrpc_core::Result<String>;
}

pub trait MetricsService {
    fn gather() -> Result<String, Error> {
        let metric_families = prometheus::gather();
        TextEncoder::new().encode_to_string(&metric_families)
    }

    #[cfg(feature = "server")]
    fn serve(
        &self,
        bind: impl Into<std::net::SocketAddr>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> {
        use hyper::{
            service::{make_service_fn, service_fn},
            Body, Method, Request, Response, Server, StatusCode,
        };
        use std::convert::Infallible;

        let addr = bind.into();
        log::info!("running metrics endpoint at http://{addr}...");

        let new_service = make_service_fn(|_| async {
            Ok::<_, Infallible>(service_fn(|req: Request<Body>| async move {
                log::info!("request {} {}", req.method(), req.uri().path());
                if req.method() != Method::GET || req.uri().path() != "/" {
                    return Ok::<_, Infallible>(
                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::empty())
                            .unwrap(),
                    );
                }

                let metrics = Self::gather().unwrap();
                let reply = Response::builder()
                    .header("Content-Type", "text/plain; version=0.0.4")
                    .body(Body::from(metrics))
                    .unwrap();
                Ok(reply)
            }))
        });

        let server = Server::bind(&addr).serve(new_service);

        Box::pin(async move {
            if let Err(error) = server.await {
                log::error!("error serving metrics: {error}");
            }
        })
    }
}
