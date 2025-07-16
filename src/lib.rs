use {
    crate::{
        exports::wasi::http::handler::Guest,
        wasi::http::types::{ErrorCode, Fields, Request, Response},
    },
    wit_bindgen_rt::async_support,
};

wit_bindgen::generate!({
    path: "wit",
    world: "wasi:http/proxy@0.3.0-draft",
    generate_all,
});

struct Component;

export!(Component);

impl Guest for Component {
    async fn handle(_request: Request) -> Result<Response, ErrorCode> {
        let (_, trailers_rx) = wit_future::new(|| Ok(None));
        let (mut content_tx, content_rx) = wit_stream::new();

        async_support::spawn(async move {
            content_tx
                .write_all(b"Hello, wasi:http/proxy world!\n".to_vec())
                .await;
        });

        Ok(Response::new(Fields::new(), Some(content_rx), trailers_rx).0)
    }
}
