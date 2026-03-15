//! 飞书长连接 WebSocket 帧格式（pbbp2），与 larksuite/oapi-sdk-python 一致。
//! 手写结构体，与 feishu_ws_frame.proto 定义对应，不再依赖 build 时代码生成。

pub mod pbbp2 {
    use prost::Message;

    #[derive(Clone, PartialEq, Message)]
    pub struct Header {
        #[prost(string, tag = "1")]
        pub key: String,
        #[prost(string, tag = "2")]
        pub value: String,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Frame {
        #[prost(uint64, tag = "1")]
        pub seq_id: u64,
        #[prost(uint64, tag = "2")]
        pub log_id: u64,
        #[prost(int32, tag = "3")]
        pub service: i32,
        #[prost(int32, tag = "4")]
        pub method: i32,
        #[prost(message, repeated, tag = "5")]
        pub headers: Vec<Header>,
        #[prost(string, tag = "6")]
        pub payload_encoding: String,
        #[prost(string, tag = "7")]
        pub payload_type: String,
        #[prost(bytes, tag = "8")]
        pub payload: Vec<u8>,
        #[prost(string, tag = "9")]
        pub log_id_new: String,
    }
}
