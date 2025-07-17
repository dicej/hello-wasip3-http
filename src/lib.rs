#![deny(warnings)]

wit_bindgen::generate!({
    path: "wit",
    world: "wasi:http/proxy@0.3.0-draft",
    generate_all,
});

static MESSAGE: &[u8] = b"Hello, wasi:http/proxy world!\n";

#[cfg(not(feature = "raw"))]
mod imp {
    use {
        super::{
            MESSAGE,
            exports::wasi::http::handler::Guest,
            wasi::http::types::{ErrorCode, Fields, Request, Response},
            wit_future, wit_stream,
        },
        wit_bindgen_rt::async_support,
    };

    struct Component;

    super::export!(Component with_types_in super);

    impl Guest for Component {
        async fn handle(_request: Request) -> Result<Response, ErrorCode> {
            let (mut content_tx, content_rx) = wit_stream::new();

            async_support::spawn(async move {
                content_tx.write_all(MESSAGE.to_vec()).await;
            });

            Ok(Response::new(
                Fields::new(),
                Some(content_rx),
                wit_future::new(|| Ok(None)).1,
            )
            .0)
        }
    }
}

#[cfg(feature = "raw")]
mod imp {
    use {
        super::{
            MESSAGE,
            wasi::http::types::{Fields, Response},
            wit_future, wit_stream,
        },
        std::{
            mem::{self, MaybeUninit},
            ptr,
        },
        wit_bindgen_rt::async_support::{FutureReader, StreamReader},
    };

    const EVENT_FUTURE_WRITE: u32 = 5;
    pub const EVENT_STREAM_WRITE: u32 = 3;

    const CALLBACK_CODE_EXIT: u32 = 0;
    const CALLBACK_CODE_WAIT: u32 = 2;

    const BLOCKED: u32 = 0xffff_ffff;
    const COMPLETED: u32 = 0x0;

    #[link(wasm_import_module = "$root")]
    unsafe extern "C" {
        #[link_name = "[context-get-0]"]
        fn context_get() -> u32;
    }

    #[link(wasm_import_module = "$root")]
    unsafe extern "C" {
        #[link_name = "[context-set-0]"]
        fn context_set(value: u32);
    }

    #[link(wasm_import_module = "$root")]
    unsafe extern "C" {
        #[link_name = "[waitable-set-new]"]
        fn waitable_set_new() -> u32;
    }

    #[link(wasm_import_module = "$root")]
    unsafe extern "C" {
        #[link_name = "[waitable-join]"]
        fn waitable_join(waitable: u32, set: u32);
    }

    #[link(wasm_import_module = "$root")]
    unsafe extern "C" {
        #[link_name = "[waitable-set-drop]"]
        pub fn waitable_set_drop(set: u32);
    }

    #[link(wasm_import_module = "[export]wasi:http/handler@0.3.0-draft")]
    unsafe extern "C" {
        #[link_name = "[task-return][async]handle"]
        fn task_return_handle(
            _: i32,
            _: i32,
            _: i32,
            _: mem::MaybeUninit<u64>,
            _: *mut u8,
            _: *mut u8,
            _: usize,
            _: i32,
        );
    }

    struct State {
        set: u32,
        event_count: u32,
    }

    #[unsafe(export_name = "[async-lift]wasi:http/handler@0.3.0-draft#[async]handle")]
    unsafe extern "C" fn export_async_handle(_request: i32) -> u32 {
        unsafe {
            let set = waitable_set_new();
            context_set(
                u32::try_from(Box::into_raw(Box::new(State {
                    set,
                    event_count: 0,
                })) as usize)
                .unwrap(),
            );

            let vtable = &wit_future::vtable0::VTABLE;
            let handles = (vtable.new)();
            let reader = handles as u32;
            let writer = (handles >> 32) as u32;
            let ok_none = [0u64; 5];
            let code = (vtable.start_write)(writer, (&raw const ok_none).cast());
            assert_eq!(code, BLOCKED);
            waitable_join(writer, set);
            let trailers = FutureReader::new(reader, vtable);

            let vtable = &wit_stream::vtable0::VTABLE;
            let handles = (vtable.new)();
            let reader = handles as u32;
            let writer = (handles >> 32) as u32;
            let code = (vtable.start_write)(writer, MESSAGE.as_ptr(), MESSAGE.len());
            assert_eq!(code, BLOCKED);
            waitable_join(writer, set);
            let contents = StreamReader::new(reader, vtable);

            let response = Response::new(Fields::new(), Some(contents), trailers).0;

            task_return_handle(
                0,
                response.take_handle() as i32,
                0,
                MaybeUninit::zeroed(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                0,
            );

            CALLBACK_CODE_WAIT | (set << 4)
        }
    }

    #[unsafe(export_name = "[callback][async-lift]wasi:http/handler@0.3.0-draft#[async]handle")]
    unsafe extern "C" fn _callback_async_handle(event0: u32, event1: u32, event2: u32) -> u32 {
        unsafe {
            let state_ptr = usize::try_from(context_get()).unwrap() as *mut State;
            let state = &mut *state_ptr;
            state.event_count += 1;
            match event0 {
                EVENT_STREAM_WRITE => {
                    assert_eq!(
                        event2,
                        COMPLETED | (u32::try_from(MESSAGE.len()).unwrap() << 4)
                    );
                    waitable_join(event1, 0);
                    (wit_stream::vtable0::VTABLE.drop_writable)(event1);
                }
                EVENT_FUTURE_WRITE => {
                    assert_eq!(event2, COMPLETED);
                    waitable_join(event1, 0);
                    (wit_future::vtable0::VTABLE.drop_writable)(event1);
                }
                _ => unreachable!(),
            }

            if state.event_count == 2 {
                waitable_set_drop(state.set);
                drop(Box::from_raw(state_ptr));
                CALLBACK_CODE_EXIT
            } else {
                CALLBACK_CODE_WAIT | (state.set << 4)
            }
        }
    }
}
